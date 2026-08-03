// Integration test for issue #37: simulates a *live* Redis connection dying
// mid-operation (not just being unreachable at connect time -- that's already
// covered by redis_startup_test.rs) and asserts RedisRateLimiter falls back to its
// in-memory per-instance shadow on every RateLimiter method (see #89), while
// incrementing its error counter each time.
//
// To run: cargo test --test integration -- --ignored redis_connection_failure
//
// Prerequisites:
// - `podman` (or an aliased `docker`) available on PATH and able to pull/run images
//   without sudo, matching the existing run-postgres-dev.ps1-style local setup.
//
// This test is fully self-contained: it starts its own disposable Redis container on
// an OS-assigned free port (so it never collides with a dev/test Redis you might
// already have running -- e.g. scripts/test-with-podman.ps1's compose setup binds
// port 16379, which is why this test does NOT hardcode that port), connects to it,
// kills it mid-test to simulate a live failure, and removes the container when done
// (via RedisTestContainer's Drop impl, which runs even if an assertion panics, since
// panics unwind by default).

use std::net::TcpListener;
use std::process::Command;
use std::time::Duration;

use app_home_services::infrastructure::rate_limiter::redis::RedisRateLimiter;
use app_home_services::shared::ports::RateLimiter;

/// Asks the OS for a free TCP port by binding to port 0 and reading back what it
/// assigned, then immediately releasing it. There's an inherent small race between
/// releasing the port here and `podman run` binding it moments later, but this is
/// the standard lightweight way to avoid hardcoding a port that might already be in
/// use by something else on the machine (as `16379` was, colliding with
/// scripts/test-with-podman.ps1's own Redis container).
fn find_free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("failed to bind to an ephemeral port to find a free one")
        .local_addr()
        .expect("failed to read the bound ephemeral port")
        .port()
}

/// Manages a disposable `redis:7-alpine` container for this test only, via `podman`.
/// Names the container using this process's PID so two test runs never collide, and
/// removes it (forcefully, in case it was already killed by the test) on drop.
struct RedisTestContainer {
    name: String,
    port: u16,
}

impl RedisTestContainer {
    fn start() -> Self {
        let name = format!("apphome-redis-flaky-test-{}", std::process::id());
        let port = find_free_port();

        // Best-effort cleanup of a leftover container from a previous crashed run
        // with the same PID (unlikely, but cheap to guard against).
        let _ = Command::new("podman").args(["rm", "-f", &name]).output();

        let status = Command::new("podman")
            .args([
                "run",
                "-d",
                "--name",
                &name,
                "-p",
                &format!("{port}:6379"),
                "docker.io/library/redis:7-alpine",
            ])
            .status()
            .expect("failed to run `podman run` -- is podman installed and on PATH?");

        assert!(
            status.success(),
            "`podman run` failed to start the test Redis container"
        );

        let container = Self { name, port };
        container.wait_until_ready();
        container
    }

    fn redis_url(&self) -> String {
        // `localhost`, not `127.0.0.1`: podman's port forwarding on Windows/WSL
        // relays to the IPv6 loopback (::1), and `127.0.0.1` is unreachable there.
        format!("redis://localhost:{}", self.port)
    }

    /// Polls with a real Redis connection attempt (not just a TCP port check) until
    /// the container is actually accepting Redis commands, since `podman run -d`
    /// returns as soon as the container process starts, not once Redis inside it is
    /// ready to serve.
    fn wait_until_ready(&self) {
        let addr = self.redis_url();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);

        loop {
            let ready = std::thread::spawn({
                let addr = addr.clone();
                move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        redis::Client::open(addr.as_str())
                            .ok()?
                            .get_connection_manager()
                            .await
                            .ok()
                    })
                }
            })
            .join()
            .expect("readiness check thread panicked")
            .is_some();

            if ready {
                return;
            }

            assert!(
                std::time::Instant::now() < deadline,
                "test Redis container never became ready within 15s"
            );
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    /// Kills the container immediately (SIGKILL, no graceful shutdown grace period)
    /// to simulate a Redis process dying outright while a client is connected to it,
    /// rather than a graceful `podman stop` which sends SIGTERM first.
    fn kill(&self) {
        let status = Command::new("podman")
            .args(["kill", &self.name])
            .status()
            .expect("failed to run `podman kill`");

        assert!(
            status.success(),
            "`podman kill` failed on the test container"
        );
    }
}

impl Drop for RedisTestContainer {
    fn drop(&mut self) {
        // Force-remove regardless of whether it's already dead (post-kill) or still
        // running (if the test panicked before calling kill()) -- ignore the result,
        // since there's nothing more useful to do with a cleanup failure here than
        // let it surface as leftover container noise the next time `podman ps -a` is
        // run.
        let _ = Command::new("podman")
            .args(["rm", "-f", &self.name])
            .output();
    }
}

#[tokio::test]
#[ignore]
async fn redis_connection_failure_causes_every_method_to_use_the_shadow() {
    use std::net::{IpAddr, Ipv4Addr};

    let container = RedisTestContainer::start();
    let redis_url = container.redis_url();

    let limiter = RedisRateLimiter::connect(&redis_url, 10, 300, "flaky-test")
        .await
        .expect("initial connection to the healthy test container should succeed");

    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    // Sanity check against the still-healthy container: no errors recorded yet, and
    // a normal (non-shadow) check succeeds because there's simply no counter
    // for this IP yet, not because of a degraded path.
    assert!(limiter.check(ip).await);
    assert_eq!(
        limiter.redis_error_count(),
        0,
        "no Redis errors should be recorded before the container is killed"
    );

    container.kill();

    // Give the OS/TCP stack a brief moment to actually notice the peer is gone
    // (SIGKILL followed immediately by a command can occasionally still hit an
    // OS-buffered "looks writable" state on some platforms before the connection
    // reset is observed).
    tokio::time::sleep(Duration::from_millis(500)).await;

    // check(): must not hang or propagate the error; the shadow (fresh IP) allows.
    assert!(
        limiter.check(ip).await,
        "check() must allow a fresh IP via the in-memory shadow when Redis is unreachable"
    );
    assert!(
        limiter.redis_error_count() >= 1,
        "check() against a dead connection should have incremented the error counter"
    );

    // record_attempt(): must not panic and must still increment the error counter,
    // recording the attempt in the shadow.
    let count_before_record = limiter.redis_error_count();
    limiter.record_attempt(ip).await;
    assert!(
        limiter.redis_error_count() > count_before_record,
        "record_attempt() against a dead connection should have incremented the error counter"
    );

    // remaining_attempts(): reads the shadow budget (10 max, 1 attempt recorded
    // above), not a fail-open max that would hide that enforcement is active.
    assert_eq!(
        limiter.remaining_attempts(ip).await,
        9,
        "remaining_attempts() must report the shadow budget, not the full max, once the shadow is enforcing"
    );

    // reset(): must not panic, should still count as an observed error, and clears
    // the shadow entry so the budget for this IP is fresh again.
    let count_before_reset = limiter.redis_error_count();
    limiter.reset(ip).await;
    assert!(
        limiter.redis_error_count() > count_before_reset,
        "reset() against a dead connection should have incremented the error counter"
    );
    assert_eq!(
        limiter.remaining_attempts(ip).await,
        10,
        "reset() must clear the shadow entry so the budget is fresh"
    );
}

#[tokio::test]
#[ignore]
async fn redis_connection_failure_shadow_still_enforces_the_per_ip_budget() {
    use std::net::{IpAddr, Ipv4Addr};

    let container = RedisTestContainer::start();
    let redis_url = container.redis_url();

    let max_attempts = 10;
    let limiter = RedisRateLimiter::connect(&redis_url, max_attempts, 300, "flaky-budget")
        .await
        .expect("initial connection to the healthy test container should succeed");

    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    container.kill();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // While Redis is down, try_check_and_record must still enforce the per-IP
    // budget via the in-memory shadow: the first max_attempts are allowed, the
    // next one is rejected. This is the regression test for #89.
    for allowed in 1..=max_attempts {
        assert!(
            limiter.try_check_and_record(ip).await,
            "attempt {allowed}/{max_attempts} should be allowed by the shadow budget"
        );
    }
    assert!(
        !limiter.try_check_and_record(ip).await,
        "the {}-th attempt must be rejected once the shadow budget is exhausted",
        max_attempts + 1
    );
    assert!(
        !limiter.check(ip).await,
        "check() must report the IP as rate-limited via the shadow budget"
    );
    assert_eq!(
        limiter.remaining_attempts(ip).await,
        0,
        "remaining_attempts() must report 0 when the shadow budget is exhausted"
    );

    // A successful login resets the counter: after reset, the shadow budget is
    // fresh again even though Redis is still down.
    limiter.reset(ip).await;
    assert!(
        limiter.try_check_and_record(ip).await,
        "the shadow budget must be fresh again after reset()"
    );
    assert!(
        limiter.redis_error_count() >= 1,
        "the outage should have incremented the error counter"
    );
}
