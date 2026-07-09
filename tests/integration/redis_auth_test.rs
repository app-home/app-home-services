// Integration test for issue #38: exercises RedisRateLimiter::connect against a
// password-protected Redis, covering both a successful authenticated connection and
// a rejected one (wrong/missing password), to confirm auth is actually enforced
// rather than silently ignored.
//
// To run: cargo test --test integration -- --ignored redis_auth
//
// Prerequisites:
// - `podman` (or an aliased `docker`) available on PATH and able to pull/run images
//   without sudo, matching the existing run-postgres-dev.ps1-style local setup.
//
// This test is fully self-contained: it starts its own disposable Redis container
// (configured with `--requirepass`) on an OS-assigned free port, and removes it when
// done via AuthedRedisTestContainer's Drop impl.
//
// See docs/redis-security.md for why there's no equivalent TLS test in this file --
// short version: this deployment doesn't terminate TLS at the `redis` crate today,
// so a test exercising `rediss://` wouldn't reflect how Redis is actually reached in
// production, and would need a new Cargo feature + certificate setup to mean
// anything. See that doc for the full reasoning and the trigger for revisiting it.

use std::net::TcpListener;
use std::process::Command;
use std::time::Duration;

use app_home_services::adapters::outbound::redis_rate_limiter::RedisRateLimiter;

const TEST_PASSWORD: &str = "test-password-for-apphome-redis-auth-check";

fn find_free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("failed to bind to an ephemeral port to find a free one")
        .local_addr()
        .expect("failed to read the bound ephemeral port")
        .port()
}

/// Manages a disposable, password-protected `redis:7-alpine` container for this test
/// only, via `podman`. Names the container using this process's PID so two test runs
/// never collide, and force-removes it on drop.
struct AuthedRedisTestContainer {
    name: String,
    port: u16,
}

impl AuthedRedisTestContainer {
    fn start() -> Self {
        let name = format!("apphome-redis-auth-test-{}", std::process::id());
        let port = find_free_port();

        let _ = Command::new("podman").args(["rm", "-f", &name]).output();

        let status = Command::new("podman")
            .args([
                "run",
                "-d",
                "--name",
                &name,
                "-p",
                &format!("{port}:6379"),
                "docker.io/redis:7-alpine",
                "redis-server",
                "--requirepass",
                TEST_PASSWORD,
            ])
            .status()
            .expect("failed to run `podman run` -- is podman installed and on PATH?");

        assert!(
            status.success(),
            "`podman run` failed to start the password-protected test Redis container"
        );

        let container = Self { name, port };
        container.wait_until_ready();
        container
    }

    fn url_with_password(&self, password: &str) -> String {
        format!("redis://:{password}@127.0.0.1:{}", self.port)
    }

    fn url_without_password(&self) -> String {
        format!("redis://127.0.0.1:{}", self.port)
    }

    /// Polls with a real, correctly-authenticated connection attempt until the
    /// container is actually accepting Redis commands, since `podman run -d` returns
    /// as soon as the container process starts, not once Redis inside it is ready.
    fn wait_until_ready(&self) {
        let addr = self.url_with_password(TEST_PASSWORD);
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
}

impl Drop for AuthedRedisTestContainer {
    fn drop(&mut self) {
        let _ = Command::new("podman")
            .args(["rm", "-f", &self.name])
            .output();
    }
}

#[tokio::test]
#[ignore]
async fn connects_successfully_with_the_correct_password() {
    let container = AuthedRedisTestContainer::start();

    let result = RedisRateLimiter::connect(
        &container.url_with_password(TEST_PASSWORD),
        10,
        300,
        "auth-test-correct",
    )
    .await;

    assert!(
        result.is_ok(),
        "connecting with the correct password should succeed, got {result:?}"
    );
}

#[tokio::test]
#[ignore]
async fn rejects_connection_with_the_wrong_password() {
    let container = AuthedRedisTestContainer::start();

    let result = RedisRateLimiter::connect(
        &container.url_with_password("definitely-the-wrong-password"),
        10,
        300,
        "auth-test-wrong",
    )
    .await;

    assert!(
        result.is_err(),
        "connecting with the wrong password must fail -- if this succeeds, auth \
         enforcement is broken (or not actually enabled on the server)"
    );
}

#[tokio::test]
#[ignore]
async fn rejects_connection_with_no_password_at_all() {
    let container = AuthedRedisTestContainer::start();

    let result =
        RedisRateLimiter::connect(&container.url_without_password(), 10, 300, "auth-test-none")
            .await;

    assert!(
        result.is_err(),
        "connecting with no password to a --requirepass server must fail"
    );
}
