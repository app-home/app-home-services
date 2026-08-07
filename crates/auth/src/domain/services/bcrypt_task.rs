use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::domain::errors::AuthError;

/// Default process-wide cap on concurrent bcrypt hash/verify operations (see
/// #175). bcrypt is deliberately slow and single-threaded per call; this default
/// is a starting point for a small instance (a handful of CPU cores) that allows
/// some overlap -- most bcrypt calls are independent per-request work, so a
/// little concurrency helps throughput -- without letting a burst of concurrent
/// logins fully saturate every core at once. Override via
/// `BCRYPT_MAX_CONCURRENT` for a deployment with more cores or a different
/// latency/throughput tradeoff.
pub const DEFAULT_BCRYPT_MAX_CONCURRENT: usize = 8;

/// Runs bcrypt hash/verify work off the async runtime's own worker threads,
/// bounded by a shared, process-wide concurrency limit.
///
/// bcrypt is synchronous and CPU-bound -- that's what makes it a suitable
/// password hash, but it also means calling it inline from an `async fn` blocks
/// whichever Tokio worker thread happens to be running that task for the whole
/// operation, starving every other task scheduled on that same thread (see
/// #175). `run_bounded` moves the work onto `spawn_blocking`'s dedicated
/// blocking-thread pool instead, so it never occupies an async worker thread.
///
/// The semaphore exists because per-IP rate limiting alone doesn't bound bcrypt
/// work process-wide: many different IPs, each individually within their own
/// rate limit, can still trigger unbounded *concurrent* bcrypt calls. Cloning a
/// `BcryptLimiter` is cheap (an `Arc` clone) and every clone shares the same
/// underlying semaphore, so all of `AuthSettings`'s clones (see
/// `AuthSettings::bcrypt_limiter`) enforce one process-wide limit together, not
/// one limit per clone.
#[derive(Clone)]
pub struct BcryptLimiter {
    semaphore: Arc<Semaphore>,
}

impl BcryptLimiter {
    /// `max_concurrent` is the process-wide cap on simultaneous bcrypt
    /// operations (hash or verify, combined); see `DEFAULT_BCRYPT_MAX_CONCURRENT`
    /// for the default and its rationale. Clamped to at least 1 --
    /// `Semaphore::new(0)` would permanently block every bcrypt call, which is
    /// never a sensible outcome for a misconfigured value.
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent.max(1))),
        }
    }

    /// Acquires one permit (waiting, not busy-polling, if the limit is currently
    /// reached -- this suspends the calling task without blocking the Tokio
    /// worker thread it's running on) and runs the CPU-bound `f` on a
    /// `spawn_blocking` thread while holding it.
    ///
    /// Returns `Err(AuthError::InternalError)` only for the two failure modes
    /// that are about the execution mechanism, not about `f` itself: the
    /// semaphore being closed (never happens in practice -- nothing ever calls
    /// `.close()` on it) and the spawned blocking task panicking or being
    /// cancelled. `f`'s own return value -- including any `Result` or `bool` it
    /// computes -- passes through unchanged as the `Ok(T)` payload; callers
    /// handle `f`'s own errors exactly as they would have inline.
    pub async fn run_bounded<F, T>(&self, f: F) -> Result<T, AuthError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let permit = self.semaphore.clone().acquire_owned().await.map_err(|_| {
            AuthError::InternalError("bcrypt concurrency semaphore was unexpectedly closed".into())
        })?;

        tokio::task::spawn_blocking(move || {
            let _permit = permit; // held until f() returns
            f()
        })
        .await
        .map_err(|e| AuthError::InternalError(format!("bcrypt task failed to complete: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn runs_work_and_returns_its_result() {
        let limiter = BcryptLimiter::new(4);

        let result = limiter.run_bounded(|| 2 + 2).await.unwrap();

        assert_eq!(result, 4);
    }

    #[tokio::test]
    async fn passes_through_the_closures_own_result_type_unchanged() {
        let limiter = BcryptLimiter::new(4);

        let ok: Result<u32, String> = limiter.run_bounded(|| Ok(42)).await.unwrap();
        let err: Result<u32, String> = limiter
            .run_bounded(|| Err("bcrypt error".to_string()))
            .await
            .unwrap();

        assert_eq!(ok, Ok(42));
        assert_eq!(err, Err("bcrypt error".to_string()));
    }

    #[tokio::test]
    async fn a_panicking_task_surfaces_as_err_instead_of_propagating() {
        let limiter = BcryptLimiter::new(4);

        let result: Result<(), AuthError> = limiter
            .run_bounded(|| panic!("simulated bcrypt task panic"))
            .await;

        assert!(
            result.is_err(),
            "a panicking blocking task must surface as Err, not crash the caller"
        );
    }

    #[tokio::test]
    async fn zero_is_clamped_to_a_working_limit_of_one() {
        let limiter = BcryptLimiter::new(0);

        // Would hang forever (never acquiring a permit) if the clamp didn't apply.
        let result = tokio::time::timeout(Duration::from_secs(5), limiter.run_bounded(|| 1))
            .await
            .expect("run_bounded should not hang when constructed with max_concurrent=0");

        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn bounds_concurrent_executions_to_the_configured_limit() {
        const MAX_CONCURRENT: usize = 3;
        const TASKS: usize = 10;

        let limiter = BcryptLimiter::new(MAX_CONCURRENT);
        let current = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::with_capacity(TASKS);
        for _ in 0..TASKS {
            let limiter = limiter.clone();
            let current = Arc::clone(&current);
            let peak = Arc::clone(&peak);
            handles.push(tokio::spawn(async move {
                limiter
                    .run_bounded(move || {
                        let now = current.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(50));
                        current.fetch_sub(1, Ordering::SeqCst);
                    })
                    .await
                    .unwrap();
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let observed_peak = peak.load(Ordering::SeqCst);
        assert!(
            observed_peak <= MAX_CONCURRENT,
            "observed {observed_peak} concurrent executions, expected at most {MAX_CONCURRENT}"
        );
    }

    #[tokio::test]
    async fn clones_share_the_same_underlying_limit() {
        // AuthSettings::clone() (and every clone of it) must enforce ONE
        // process-wide limit together, not a fresh limit per clone -- see the
        // struct docs.
        const MAX_CONCURRENT: usize = 2;
        const TASKS: usize = 6;

        let original = BcryptLimiter::new(MAX_CONCURRENT);
        let current = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::with_capacity(TASKS);
        for _ in 0..TASKS {
            // Each task uses its own clone, simulating separate AuthSettings
            // clones handed to concurrent request handlers.
            let limiter = original.clone();
            let current = Arc::clone(&current);
            let peak = Arc::clone(&peak);
            handles.push(tokio::spawn(async move {
                limiter
                    .run_bounded(move || {
                        let now = current.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(50));
                        current.fetch_sub(1, Ordering::SeqCst);
                    })
                    .await
                    .unwrap();
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let observed_peak = peak.load(Ordering::SeqCst);
        assert!(
            observed_peak <= MAX_CONCURRENT,
            "clones must share one limit: observed {observed_peak} concurrent, expected at most {MAX_CONCURRENT}"
        );
    }
}
