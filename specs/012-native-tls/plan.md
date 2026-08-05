# Implementation Plan: Optional native TLS support

**Branch**: feat/93-native-tls
**Date**: 2026-08-04
**Issue**: #93

## Summary

Issue #93 (security, Medium) reports the service has no native TLS support and
depends entirely on a reverse proxy for HTTPS. Proposed resolution (confirmed):

- **Document the TLS requirement** in the README: HTTPS in production, two
  mutually exclusive options (reverse proxy by default, native TLS optional).
- **Native TLS opt-in** via `TLS_CERT_PATH` + `TLS_KEY_PATH`: when both are
  set, the service terminates HTTPS itself using `axum-server` + `rustls`.
  Setting only one aborts startup (fail-fast, never silently serve plaintext).
- **HSTS is already emitted unconditionally** (decision from #90) and is only
  honored by browsers over real HTTPS, so no HSTS code change is needed; the
  README explains this.
- **Coverage decision**: unit tests for the pure parsing/decision logic +
  automated native-TLS smoke test in `src/security_headers.rs` (exercises
  `CryptoProvider::install_default`, `RustlsConfig::from_pem_file`,
  `axum_server::bind_rustls` and a real HTTPS request against a self-signed
  cert generated at test runtime via `rcgen`; no DB required, runs in CI) +
  the `test-with-podman.ps1` harness for plain-HTTP no-regression. A manual
  `curl -k` check was also performed during development.

## WP A — Settings: TLS env vars (HIGH)

**Files**: `crates/shared/src/config/settings.rs`,
`crates/infrastructure/src/rate_limiter_setup.rs`

- New fields `tls_cert_path: Option<String>`, `tls_key_path: Option<String>`
  (parsed from `TLS_CERT_PATH`/`TLS_KEY_PATH`, blank = `None`).
- New pure helper `parse_tls_paths(raw_cert, raw_key)` (module-level `pub`,
  same rationale as `parse_required_ssl_flag`): returns `Ok((None, None))`
  when both unset, `Ok((Some, Some))` when both set, `Err` when exactly one is
  set. `from_env` calls it so the fail-fast applies at startup.
- `Debug` impl includes both fields; `settings_with_redis_url` fixture gains
  `tls_cert_path: None, tls_key_path: None`.

## WP B — Serve: axum-server + rustls (HIGH)

**Files**: `Cargo.toml`, `src/main.rs`

- Dependency `axum-server = { version = "0.8", features = ["tls-rustls"] }`
  (compatible with axum 0.8; rustls 0.23, hyper 1.4).
- **Crypto provider**: the resolved rustls graph enables BOTH `aws-lc-rs`
  (axum-server/sqlx) and `ring` (reqwest's hyper-rustls), so rustls 0.23 cannot
  auto-select a provider and panics at first handshake. Added a direct
  `rustls = { version = "0.23", features = ["aws-lc-rs"] }` dep and call
  `CryptoProvider::install_default(aws_lc_rs::default_provider())` as the very
  first thing in `main()`.
- Load TLS config up front (`RustlsConfig::from_pem_file`, async) so a
  missing/malformed cert fails startup loudly, not at the first handshake.
- Serve branch:
  - TLS: `axum_server::bind_rustls(addr.parse()?, tls).serve(service)` — the
    crate binds via tokio internally. **Do NOT use `from_tcp_rustls` with a
    pre-bound `std::net::TcpListener`**: `tokio::net::TcpListener::from_std`
    (used by both axum-server's `from_tcp` and the plain-path conversion) was
    found to accept connections but never process HTTP on Windows (verified by
    bisect: plain path with `from_std` hung, original `tokio::...::bind`
    returned 200).
  - Plain: original `tokio::net::TcpListener::bind` + `axum::serve` (unchanged).
- Both paths share `app.into_make_service_with_connect_info::<SocketAddr>()`;
  axum-server passes each peer `SocketAddr` to the make service and axum's
  `Connected<SocketAddr>` impl produces the same `ConnectInfo<SocketAddr>`
  extension, so client-IP resolution and the `/metrics` allowlist keep working
  under TLS.
- Startup log: "Native TLS enabled (rustls)" with cert path, or "Native TLS
  disabled: TLS termination is expected from a reverse proxy...".

## WP C — Documentation (HIGH)

**Files**: `README.md`, `.env.example`

- New "HTTPS / TLS" README section: reverse proxy (default, with
  `TRUSTED_PROXY_IPS`) vs native TLS via `TLS_CERT_PATH`/`TLS_KEY_PATH`;
  HSTS note (always sent, honored only under HTTPS — see #90).
- Env var table rows for `TLS_CERT_PATH`/`TLS_KEY_PATH`.
- `.env.example`: commented TLS block with example self-signed cert generation.

## WP D — Tests + plan (MEDIUM)

**Files**: `crates/shared/tests/tls_settings_test.rs`, `src/security_headers.rs`,
`specs/012-native-tls/plan.md` (this file), `AGENTS.md`

- Unit tests on `parse_tls_paths`: both unset, both set, blank strings,
  cert-without-key error, key-without-cert error, whitespace trimming.
- Native-TLS smoke test (`src/security_headers.rs`, `#[tokio::test]`):
  - Generates a self-signed cert with `rcgen` (dev-dependency, pure Rust), writes
    it to a temp dir and loads it via `RustlsConfig::from_pem_file` (same path
    `main.rs` uses).
  - Installs the `aws_lc_rs` crypto provider (error tolerated if a previous test
    in the binary already installed a provider).
  - Serves a minimal router (the exact `apply_security_headers` layers + a
    `/api/health` route) via `axum_server::bind_rustls` on an ephemeral port
    (free port obtained from a throwaway listener that is dropped before binding,
    so the Windows `from_std` stall path is never hit).
  - Performs a real HTTPS request with `reqwest`
    (`danger_accept_invalid_certs(true)`), asserts `200` and all four security
    headers. Runs in CI, no DB/Redis required.
- The four security-header layers moved from inline `main.rs` into the reusable
  `security_headers::apply_security_headers` (public, used by both `main.rs`
  and the smoke test).
- `SERVER_HOST:SERVER_PORT` is resolved once via `tokio::net::lookup_host`
  before the serve branch, so plain-HTTP and native-TLS share the same
  hostname-resolution contract (native TLS previously `parse`d the string and
  would panic on a hostname like `localhost`).
- SPECKIT pointer in `AGENTS.md` updated to this plan.

## Validation (all green)

- `cargo build --locked`, `cargo clippy --locked --workspace --all-targets`,
  `cargo fmt --all --check`, `cargo test --locked --workspace`.
- `scripts/test-with-podman.ps1 -IntegrationOnly`: **53/53** integration tests
  pass on the plain-HTTP path (no regression), incl. `security_headers_test`.
- Automated native-TLS smoke test (`native_tls_smoke_test`) passes as part of
  `cargo test` (no DB/Redis needed), asserting `200` + all security headers
  over a real HTTPS connection with a runtime-generated self-signed cert.
- Manual native-TLS smoke test: self-signed cert generated on Windows
  (`New-SelfSignedCertificate` + .NET PEM export, since no openssl), server run
  with `TLS_CERT_PATH`/`TLS_KEY_PATH` → `curl -k https://localhost:3000/api/health`
  returned `200` `{"status":"ok",...}` with the security headers present.
- Fail-fast: `TLS_CERT_PATH` alone → startup aborts before DB pool creation.

## Notes

- Issue #93 auto-closes only when the PR reaches `main`, so a
  `development` → `main` promotion is required at the end.
