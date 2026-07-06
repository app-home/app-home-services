// Timing-safety tests for issue #10: username enumeration via login timing.
//
// These measure wall-clock time of `verify_password_timing_safe` directly (rather
// than the full `login_with_password` use case) to isolate exactly the property that
// matters here -- CPU cost of the password-verification step -- without confounding
// noise from a database round trip, which would need to be mocked out anyway.
//
// bcrypt is deliberately slow (that's the point), so these run a modest number of
// iterations rather than being marked #[ignore]: slow enough to be meaningful, fast
// enough to run in a normal `cargo test`.

use std::time::{Duration, Instant};

use app_home_services::application::use_cases::login_with_password::verify_password_timing_safe;
use app_home_services::domain::entities::user::User;

const ITERATIONS: u32 = 8;
// Both paths perform exactly one bcrypt::verify at the same cost factor, so their
// average timing should be close. This tolerance is generous on purpose: the property
// under test is "same order of magnitude", not "identical to the microsecond", since
// OS scheduling and CI-runner jitter are real even for a pure CPU-bound operation.
const MAX_RELATIVE_DIFFERENCE: f64 = 0.5;

fn make_user(password_hash: Option<String>) -> User {
    User {
        id: uuid::Uuid::now_v7(),
        username: Some("alice".to_string()),
        email: "alice@example.com".to_string(),
        display_name: "Alice".to_string(),
        password_hash,
        auth_provider: "local".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

/// Average wall-clock time, in seconds, of `ITERATIONS` calls to `f`.
fn average_duration(mut f: impl FnMut() -> bool, expected: bool) -> f64 {
    let total: Duration = (0..ITERATIONS)
        .map(|_| {
            let start = Instant::now();
            let result = f();
            let elapsed = start.elapsed();
            assert_eq!(result, expected);
            elapsed
        })
        .sum();

    total.as_secs_f64() / f64::from(ITERATIONS)
}

fn assert_similar_timing(label_a: &str, avg_a: f64, label_b: &str, avg_b: f64) {
    let ratio = (avg_a - avg_b).abs() / avg_a.max(avg_b);
    assert!(
        ratio < MAX_RELATIVE_DIFFERENCE,
        "timing difference too large between {label_a} ({avg_a:.4}s) and {label_b} ({avg_b:.4}s): ratio={ratio:.2}"
    );
}

#[test]
fn username_not_found_and_wrong_password_take_similar_time() {
    let real_hash = bcrypt::hash("correct-password", bcrypt::DEFAULT_COST).unwrap();
    let existing_user = make_user(Some(real_hash));

    let wrong_password_avg = average_duration(
        || verify_password_timing_safe(Some(&existing_user), "wrong-password"),
        false,
    );
    let not_found_avg =
        average_duration(|| verify_password_timing_safe(None, "wrong-password"), false);

    assert_similar_timing(
        "wrong password (user exists)",
        wrong_password_avg,
        "username not found",
        not_found_avg,
    );
}

#[test]
fn google_only_account_and_wrong_password_take_similar_time() {
    // A user that exists but has no password set (e.g. only ever logged in via
    // Google) must not be distinguishable from a real wrong-password rejection.
    let real_hash = bcrypt::hash("correct-password", bcrypt::DEFAULT_COST).unwrap();
    let user_with_password = make_user(Some(real_hash));
    let google_only_user = make_user(None);

    let wrong_password_avg = average_duration(
        || verify_password_timing_safe(Some(&user_with_password), "wrong-password"),
        false,
    );
    let google_only_avg = average_duration(
        || verify_password_timing_safe(Some(&google_only_user), "anything"),
        false,
    );

    assert_similar_timing(
        "wrong password (user exists)",
        wrong_password_avg,
        "user exists, no password set",
        google_only_avg,
    );
}

#[test]
fn correct_password_still_verifies_successfully() {
    // Sanity check: closing the timing side-channel must not break real,
    // successful password verification, nor accidentally accept wrong passwords.
    let real_hash = bcrypt::hash("correct-password", bcrypt::DEFAULT_COST).unwrap();
    let user = make_user(Some(real_hash));

    assert!(verify_password_timing_safe(
        Some(&user),
        "correct-password"
    ));
    assert!(!verify_password_timing_safe(Some(&user), "wrong-password"));
    assert!(!verify_password_timing_safe(None, "correct-password"));
}
