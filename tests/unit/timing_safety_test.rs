use std::time::{Duration, Instant};

use app_home_services::domain::entities::user::User;
use app_home_services::domain::services::password_verification::verify_password_timing_safe;
use shared::domain::value_objects::auth_provider::AuthProvider;
use shared::domain::value_objects::email::Email;
use shared::domain::value_objects::hashed_password::HashedPassword;

const ITERATIONS: u32 = 8;
const MAX_RELATIVE_DIFFERENCE: f64 = 0.5;

fn make_user(password_hash: Option<HashedPassword>) -> User {
    let email = Email::new("alice@example.com").unwrap();
    let auth_provider = if password_hash.is_some() {
        AuthProvider::Local
    } else {
        AuthProvider::Google
    };
    User::new(
        uuid::Uuid::now_v7(),
        Some("alice".to_string()),
        email,
        "Alice".to_string(),
        password_hash,
        auth_provider,
        chrono::Utc::now(),
        chrono::Utc::now(),
    )
}

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
    let existing_user = make_user(Some(HashedPassword::new(real_hash).unwrap()));

    let wrong_password_avg = average_duration(
        || verify_password_timing_safe(Some(&existing_user), "wrong-password"),
        false,
    );
    let not_found_avg = average_duration(
        || verify_password_timing_safe(None, "wrong-password"),
        false,
    );

    assert_similar_timing(
        "wrong password (user exists)",
        wrong_password_avg,
        "username not found",
        not_found_avg,
    );
}

#[test]
fn google_only_account_and_wrong_password_take_similar_time() {
    let real_hash = bcrypt::hash("correct-password", bcrypt::DEFAULT_COST).unwrap();
    let user_with_password = make_user(Some(HashedPassword::new(real_hash).unwrap()));
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
    let real_hash = bcrypt::hash("correct-password", bcrypt::DEFAULT_COST).unwrap();
    let user = make_user(Some(HashedPassword::new(real_hash).unwrap()));

    assert!(verify_password_timing_safe(Some(&user), "correct-password"));
    assert!(!verify_password_timing_safe(Some(&user), "wrong-password"));
    assert!(!verify_password_timing_safe(None, "correct-password"));
}
