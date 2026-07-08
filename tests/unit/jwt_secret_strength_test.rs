// Unit tests for issue #14: Settings::from_env() must reject a JWT_SECRET that is
// shorter than the minimum strength requirement, rather than silently accepting it
// and weakening the HS256 signing used for access and refresh tokens.
//
// These call `validate_jwt_secret` directly rather than `Settings::from_env`, since
// `from_env` reads real process environment variables -- exercising it here would
// mean mutating shared process-global state from parallel tests, which is racy.
// `validate_jwt_secret` is the pure piece of logic this issue is actually about.

use app_home_services::infrastructure::config::settings::{validate_jwt_secret, MIN_JWT_SECRET_LEN};

#[test]
fn rejects_a_trivially_short_secret() {
    let result = validate_jwt_secret("123");

    assert!(
        result.is_err(),
        "a 3-byte secret must be rejected, got {result:?}"
    );
}

#[test]
fn rejects_a_secret_one_byte_under_the_minimum() {
    let secret = "a".repeat(MIN_JWT_SECRET_LEN - 1);

    let result = validate_jwt_secret(&secret);

    assert!(
        result.is_err(),
        "a secret one byte under the minimum must be rejected, got {result:?}"
    );
}

#[test]
fn accepts_a_secret_exactly_at_the_minimum() {
    let secret = "a".repeat(MIN_JWT_SECRET_LEN);

    let result = validate_jwt_secret(&secret);

    assert!(
        result.is_ok(),
        "a secret exactly at the minimum length should be accepted, got {result:?}"
    );
}

#[test]
fn accepts_a_strong_secret() {
    // Mirrors the `.env.example` recommendation of `openssl rand -hex 64` (128 hex
    // chars / bytes as a string).
    let secret = "f".repeat(128);

    let result = validate_jwt_secret(&secret);

    assert!(result.is_ok(), "a 128-byte secret should be accepted, got {result:?}");
}

#[test]
fn rejected_secret_error_message_mentions_the_minimum_length() {
    let result = validate_jwt_secret("short");

    let err = result.expect_err("expected short secret to be rejected");
    assert!(
        err.contains(&MIN_JWT_SECRET_LEN.to_string()),
        "error message should mention the minimum length so operators know how to fix it, got: {err}"
    );
}
