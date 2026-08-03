// Unit tests for issue #11: Google login must reject unverified emails.
//
// These test `validate_google_claims` directly against hand-built claims, rather
// than a real signed Google ID token, since the business rule under test (the
// email_verified check) is independent of JWT signature verification / JWKS
// fetching, and doing it this way needs no network access to Google.

use auth::adapters::google_auth_provider::{GoogleClaims, validate_google_claims};

fn base_claims() -> GoogleClaims {
    GoogleClaims {
        sub: "1234567890".to_string(),
        email: Some("alice@example.com".to_string()),
        email_verified: Some(true),
        name: Some("Alice".to_string()),
        iss: "https://accounts.google.com".to_string(),
        aud: "test-client-id".to_string(),
        exp: 9_999_999_999,
        iat: 1,
    }
}

#[test]
fn accepts_verified_email() {
    let claims = base_claims();

    let result = validate_google_claims(claims).expect("verified email should be accepted");

    assert_eq!(result.email, "alice@example.com");
    assert_eq!(result.name, "Alice");
}

#[test]
fn rejects_explicitly_unverified_email() {
    let mut claims = base_claims();
    claims.email_verified = Some(false);

    let result = validate_google_claims(claims);

    assert!(
        result.is_err(),
        "email_verified: false must be rejected, not just falsy-ish"
    );
}

#[test]
fn rejects_missing_email_verified_claim() {
    let mut claims = base_claims();
    claims.email_verified = None;

    let result = validate_google_claims(claims);

    assert!(
        result.is_err(),
        "a missing email_verified claim must be rejected, not treated as verified"
    );
}

#[test]
fn rejects_missing_email() {
    let mut claims = base_claims();
    claims.email = None;

    let result = validate_google_claims(claims);

    assert!(result.is_err());
}

#[test]
fn falls_back_to_email_as_name_when_name_claim_is_missing() {
    let mut claims = base_claims();
    claims.name = None;

    let result = validate_google_claims(claims).expect("verified email should be accepted");

    assert_eq!(result.name, "alice@example.com");
}
