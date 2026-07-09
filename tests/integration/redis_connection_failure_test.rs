// Integration test for issue #37: simulates a *live* Redis connection dying
// mid-operation (not just being unreachable at connect time -- that's already
// covered by redis_startup_test.rs) and asserts RedisRateLimiter fails open on every
// RateLimiter method, incrementing its error counter each time.
//
// To run: cargo test --test integration -- --ignored redis_connection_failure
//
// Prerequisites:
// - `podman` (or an aliased `docker`) available on PATH and able to pull/run images
//   without sudo, matching the existing run-postgres-dev.ps1-style local setup.
// - Port 16379 free on localhost (deliberately not 6379, so this never collides
//   with a dev Redis you might already have running via run-postgres-dev.ps1).
//
// This test is fully self-contained: it starts its own disposable Redis container,
// connects to it, kills it mid-test to simulate a live failure, and removes the
// container when done (via RedisTestContainer's Drop impl, which runs even if an
// assertion panics, since panics unwind by default).

use std::process::Command;
use std::time::Duration;

use app_home_services::adapters::outbound::redis_rate_limiter::RedisRateLimiter;
use app_home_services::application::ports::rate_limiter::RateLimiter;

const TEST_REDIS_PORT: u16 = 16379;

/// Manages a disposable `redis:7-alpine` container for this test only, via `podman`.
/// Names the container using this process's PID so two test runs never collide, and
/// removes it (forcefully, in case it was already killed by the test) on drop.
struct RedisTestContainer {
    name: String,
}

impl RedisTestContainer {
    fn start() -> Self {
        let name = format!("apphome-redis-flaky-test-{}", std::process::id());

        // Best-effort cleanup of a leftover container from a previous crashed run
        // with the same PID (unlikely, but cheap to guard against).
        let _ = Command::new("podman")
            .args(["rm", "-f", &name])
            .output();

        let status = Command::new("podman")
            .args([
                "run",
                "-d",
                "--name",
                &name,
                "-p",
                &format!("{TEST_REDIS_PORT}:6379"),
                "docker.io/library/redis:7-alpine",
            ])
            .status()
            .expect("failed to run `podman run` -- is podman installed and on PATH?");

        assert!(
            status.success(),
            "`podman run` failed to start the test Redis container"
        );

        let container = Self { name };
        container.wait_until_ready();
        container
    }

    /// Polls with a real Redis connection attempt (not just a TCP port check) until
    /// the container is actually accepting Redis commands, since `podman run -d`
    /// returns as soon as the container process starts, not once Redis inside it is
    /// ready to serve.
    fn wait_until_ready(&self) {
        let addr = format!("redis://127.0.0.1:{TEST_REDIS_PORT}");
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

        assert!(status.success(), "`podman kill` failed on the test container");
    }
}

impl Drop for RedisTestContainer {
    fn drop(&mut self) {
        // Force-remove regardless of whether it's already dead (post-kill) or still
        // running (if the test panicked before calling kill()) -- ignore the result,
        // since there's nothing more useful to do with a cleanup failure here than
        // let it surface as leftover container noise the next time `podman ps -a` is
        // run.
        let _ = Command::new("podman").args(["rm", "-f", &self.name]).output();
    }
}

#[tokio::test]
#[ignore]
async fn redis_connection_failure_causes_every_method_to_fail_open() {
    use std::net::{IpAddr, Ipv4Addr};

    let container = RedisTestContainer::start();
    let redis_url = format!("redis://127.0.0.1:{TEST_REDIS_PORT}");

    let limiter = RedisRateLimiter::connect(&redis_url, 10, 300, "flaky-test")
        .await
        .expect("initial connection to the healthy test container should succeed");

    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    // Sanity check against the still-healthy container: no errors recorded yet, and
    // a normal (non-failed-open) check succeeds because there's simply no counter
    // for this IP yet, not because of a fail-open path.
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

    // check(): must fail open (return true), not propagate the error or hang.
    assert!(
        limiter.check(ip).await,
        "check() must fail open (return true) when Redis is unreachable"
    );
    assert!(
        limiter.redis_error_count() >= 1,
        "check() against a dead connection should have incremented the error counter"
    );

    // record_attempt(): must not panic and must still increment the error counter,
    // even though it has no meaningful return value to assert fail-open on directly.
    let count_before_record = limiter.redis_error_count();
    limiter.record_attempt(ip).await;
    assert!(
        limiter.redis_error_count() > count_before_record,
        "record_attempt() against a dead connection should have incremented the error counter"
    );

    // remaining_attempts(): must fail open by reporting the full budget (max_attempts
    // = 10, per the RedisRateLimiter::connect call above), not a partial/zero value
    // that would incorrectly look like the limit was already being enforced.
    assert_eq!(
        limiter.remaining_attempts(ip).await,
        10,
        "remaining_attempts() must fail open to the max budget when Redis is unreachable"
    );

    // reset(): must not panic, and should still count as an observed error even
    // though there's no return value to assert fail-open behavior on.
    let count_before_reset = limiter.redis_error_count();
    limiter.reset(ip).await;
    assert!(
        limiter.redis_error_count() > count_before_reset,
        "reset() against a dead connection should have incremented the error counter"
    );
}
