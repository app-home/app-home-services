// Tests for issue #85: `sslmode=disable` in DATABASE_URL sends credentials and
// data in plaintext over the network, and the service must not start with that
// configuration against a non-local database.
//
// These call the pure validation/rewrite helpers directly rather than
// `Settings::from_env`, since `from_env` reads real process environment
// variables -- exercising it here would mean mutating shared process-global
// state from parallel tests, which is racy (same rationale as
// `crates/auth/tests/jwt_secret_strength_test.rs`).

use shared::config::settings::{
    database_ssl_warning, force_sslmode_verify_full, validate_database_ssl,
};

// -- validate_database_ssl ----------------------------------------------------

#[test]
fn accepts_sslmode_disable_for_loopback_hosts() {
    for url in [
        "postgresql://user:pass@127.0.0.1:5432/apphome?sslmode=disable",
        "postgresql://user:pass@localhost/apphome?sslmode=disable",
        "postgresql://user:pass@[::1]:5432/apphome?sslmode=disable",
    ] {
        let result = validate_database_ssl(url);
        assert!(
            result.is_ok(),
            "sslmode=disable against a loopback host must be accepted (it never leaves the machine), got {result:?}"
        );
    }
}

#[test]
fn rejects_sslmode_disable_against_a_remote_hostname() {
    let result =
        validate_database_ssl("postgresql://user:pass@db.internal:5432/apphome?sslmode=disable");

    assert!(
        result.is_err(),
        "sslmode=disable against a remote host must be a fatal startup error, got {result:?}"
    );
}

#[test]
fn rejects_sslmode_disable_against_a_remote_ip() {
    let result =
        validate_database_ssl("postgresql://user:pass@10.0.0.5:5432/apphome?sslmode=disable");

    assert!(
        result.is_err(),
        "sslmode=disable against a remote IP must be a fatal startup error, got {result:?}"
    );
}

#[test]
fn accepts_tls_modes_against_remote_hosts() {
    for sslmode in ["require", "verify-ca", "verify-full"] {
        let result = validate_database_ssl(&format!(
            "postgresql://user:pass@db.internal:5432/apphome?sslmode={sslmode}"
        ));
        assert!(
            result.is_ok(),
            "sslmode={sslmode} against a remote host must be accepted, got {result:?}"
        );
    }
}

#[test]
fn accepts_prefer_or_missing_sslmode_against_remote_hosts() {
    // `prefer`/missing still allow a plaintext fallback, but that is reported as
    // a non-fatal warning (`database_ssl_warning`), not a startup rejection.
    for url in [
        "postgresql://user:pass@db.internal:5432/apphome?sslmode=prefer",
        "postgresql://user:pass@db.internal:5432/apphome",
    ] {
        let result = validate_database_ssl(url);
        assert!(
            result.is_ok(),
            "missing/prefer sslmode must not be fatal, got {result:?}"
        );
    }
}

#[test]
fn rejects_a_malformed_database_url() {
    let result = validate_database_ssl("not a url at all");

    assert!(
        result.is_err(),
        "an unparseable DATABASE_URL should be rejected with a clear message, got {result:?}"
    );
}

// -- database_ssl_warning -----------------------------------------------------

#[test]
fn warns_when_remote_host_has_no_sslmode() {
    let warning = database_ssl_warning("postgresql://user:pass@db.internal:5432/apphome");

    assert!(
        warning.is_some(),
        "a remote host without an explicit sslmode must produce a warning"
    );
}

#[test]
fn warns_when_remote_host_has_sslmode_prefer() {
    let warning =
        database_ssl_warning("postgresql://user:pass@db.internal:5432/apphome?sslmode=prefer");

    assert!(
        warning.is_some(),
        "sslmode=prefer against a remote host must produce a warning"
    );
}

#[test]
fn warns_when_remote_host_has_sslmode_allow() {
    // `allow` tries plaintext first and only upgrades to TLS if the server
    // demands it -- same plaintext-capable outcome as `prefer`, just with the
    // attempt order reversed, so it must warn too. See #142 review.
    let warning =
        database_ssl_warning("postgresql://user:pass@db.internal:5432/apphome?sslmode=allow");

    assert!(
        warning.is_some(),
        "sslmode=allow against a remote host must produce a warning"
    );
}

#[test]
fn does_not_warn_for_verified_tls_or_loopback_hosts() {
    for url in [
        "postgresql://user:pass@db.internal:5432/apphome?sslmode=verify-full",
        "postgresql://user:pass@db.internal:5432/apphome?sslmode=verify-ca",
        "postgresql://user:pass@db.internal:5432/apphome?sslmode=require",
        "postgresql://user:pass@db.internal:5432/apphome?sslmode=disable",
        "postgresql://user:pass@127.0.0.1:5432/apphome?sslmode=disable",
        "postgresql://user:pass@localhost/apphome",
    ] {
        let warning = database_ssl_warning(url);
        assert!(
            warning.is_none(),
            "expected no warning for {url:?}, got {warning:?}"
        );
    }
}

// -- force_sslmode_verify_full ------------------------------------------------

#[test]
fn appends_verify_full_when_sslmode_is_missing() {
    let rewritten =
        force_sslmode_verify_full("postgresql://user:pass@db.internal/apphome").unwrap();

    let parsed = url::Url::parse(&rewritten).unwrap();
    let sslmode = parsed
        .query_pairs()
        .find(|(key, _)| key == "sslmode")
        .map(|(_, value)| value.into_owned());
    assert_eq!(sslmode.as_deref(), Some("verify-full"));
}

#[test]
fn replaces_an_existing_sslmode_value() {
    for original in [
        "postgresql://user:pass@db.internal/apphome?sslmode=disable",
        "postgresql://user:pass@db.internal/apphome?sslmode=require",
        "postgresql://user:pass@db.internal/apphome?sslmode=prefer",
    ] {
        let rewritten = force_sslmode_verify_full(original).unwrap();
        let parsed = url::Url::parse(&rewritten).unwrap();
        let pairs: Vec<(String, String)> = parsed
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        assert_eq!(
            pairs
                .iter()
                .filter(|(key, _)| key == "sslmode")
                .collect::<Vec<_>>()
                .len(),
            1,
            "rewritten URL must contain exactly one sslmode parameter, got {pairs:?}"
        );
        assert_eq!(
            pairs
                .iter()
                .find(|(key, _)| key == "sslmode")
                .map(|(_, v)| v.as_str()),
            Some("verify-full"),
            "original {original:?} should have been rewritten, got {pairs:?}"
        );
    }
}

#[test]
fn preserves_other_query_parameters() {
    let rewritten = force_sslmode_verify_full(
        "postgresql://user:pass@db.internal/apphome?sslmode=disable&connect_timeout=10&application_name=apphome",
    )
    .unwrap();

    let parsed = url::Url::parse(&rewritten).unwrap();
    let pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    assert!(pairs.contains(&("connect_timeout".to_string(), "10".to_string())));
    assert!(pairs.contains(&("application_name".to_string(), "apphome".to_string())));
    assert_eq!(
        pairs
            .iter()
            .find(|(key, _)| key == "sslmode")
            .map(|(_, v)| v.as_str()),
        Some("verify-full")
    );
}

#[test]
fn forced_verify_full_url_passes_remote_host_validation() {
    let rewritten =
        force_sslmode_verify_full("postgresql://user:pass@db.internal/apphome?sslmode=disable")
            .unwrap();

    let result = validate_database_ssl(&rewritten);
    assert!(
        result.is_ok(),
        "a DB_REQUIRE_SSL-rewritten URL must pass validation, got {result:?}"
    );
    assert!(database_ssl_warning(&rewritten).is_none());
}

#[test]
fn rejects_a_malformed_url_when_forcing_ssl() {
    let result = force_sslmode_verify_full("not a url at all");

    assert!(
        result.is_err(),
        "an unparseable DATABASE_URL should be rejected with a clear message, got {result:?}"
    );
}
