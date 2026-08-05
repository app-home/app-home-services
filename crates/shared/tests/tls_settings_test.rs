// Tests for issue #93: optional native TLS. `TLS_CERT_PATH`/`TLS_KEY_PATH`
// together enable rustls HTTPS on the instance itself; setting only one is a
// misconfiguration that must abort startup rather than silently serving
// plaintext.
//
// These call the pure helper directly rather than `Settings::from_env`, since
// `from_env` reads real process environment variables -- exercising it here
// would mean mutating shared process-global state from parallel tests, which
// is racy (same rationale as `database_ssl_test.rs`).

use shared::config::settings::parse_tls_paths;

#[test]
fn both_paths_unset_disables_native_tls() {
    let result = parse_tls_paths(None, None);

    let (cert, key) = result.expect("both unset must be a valid config");
    assert!(
        cert.is_none(),
        "TLS_CERT_PATH unset must yield None, got {cert:?}"
    );
    assert!(
        key.is_none(),
        "TLS_KEY_PATH unset must yield None, got {key:?}"
    );
}

#[test]
fn both_paths_set_enables_native_tls() {
    let result = parse_tls_paths(
        Some("certs/fullchain.pem".into()),
        Some("certs/privkey.pem".into()),
    );

    let (cert, key) = result.expect("both set must be a valid config");
    assert_eq!(cert.as_deref(), Some("certs/fullchain.pem"));
    assert_eq!(key.as_deref(), Some("certs/privkey.pem"));
}

#[test]
fn empty_strings_count_as_unset() {
    let result = parse_tls_paths(Some(String::new()), Some("  ".into()));

    let (cert, key) = result.expect("blank values must be treated as unset");
    assert!(cert.is_none());
    assert!(key.is_none());
}

#[test]
fn rejects_cert_without_key() {
    let result = parse_tls_paths(Some("certs/fullchain.pem".into()), None);

    let err = result.expect_err("cert without key must be a fatal config error");
    assert!(
        err.contains("TLS_CERT_PATH") && err.contains("TLS_KEY_PATH"),
        "error should name both variables, got {err:?}"
    );
}

#[test]
fn rejects_key_without_cert() {
    let result = parse_tls_paths(None, Some("certs/privkey.pem".into()));

    let err = result.expect_err("key without cert must be a fatal config error");
    assert!(
        err.contains("TLS_KEY_PATH") && err.contains("TLS_CERT_PATH"),
        "error should name both variables, got {err:?}"
    );
}

#[test]
fn trims_whitespace_padded_paths() {
    let result = parse_tls_paths(
        Some("  certs/fullchain.pem  ".into()),
        Some("certs/privkey.pem".into()),
    );

    let (cert, key) = result.expect("whitespace-padded values must still be valid");
    assert_eq!(cert.as_deref(), Some("certs/fullchain.pem"));
    assert_eq!(key.as_deref(), Some("certs/privkey.pem"));
}
