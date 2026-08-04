# Implementation Plan: HTTP security headers

**Branch**: fix/90-security-headers
**Date**: 2026-08-03
**Issue**: #90

## Summary

Issue #90 (security scan, Medium) reported that responses carry no HTTP security
headers: missing HSTS, X-Frame-Options, X-Content-Type-Options, Referrer-Policy,
and CSP. The issue's proposed fix uses `tower-http`'s
`SetResponseHeaderLayer::overriding` to emit four headers (CSP is named only in
the impact, not in the proposed resolution).

Decisions (confirmed):
- **All four headers are emitted unconditionally.** HSTS is inert over plain
  HTTP (browsers only process it on HTTPS responses), so it is safe to send even
  when TLS is terminated by a reverse proxy ahead of this service; no new
  setting was added.
- **CSP is deliberately omitted.** This service renders no HTML except the
  Swagger UI when `ENABLE_SWAGGER=true`, which relies on inline scripts and CDN
  assets; a strict CSP would break it without adding value to a JSON API.

## WP A — Feature flag + response-header layer (HIGH)

**Files**: `Cargo.toml`, `src/main.rs`

- `Cargo.toml`: `tower-http` gains the `set-header` feature
  (`features = ["cors", "set-header"]`). Features do not change `Cargo.lock`.
- `src/main.rs`: four chained `SetResponseHeaderLayer::overriding` layers are
  applied to the whole router alongside `cors` (before `.with_state(state)`):
  - `strict-transport-security: max-age=31536000; includeSubDomains`
  - `x-content-type-options: nosniff`
  - `x-frame-options: DENY`
  - `referrer-policy: strict-origin-when-cross-origin`
- Applied to every route, including `/metrics` and the Swagger UI (the four
  headers are safe there; CSP is the only one that would have broken the UI).

## WP B — Integration test (MEDIUM)

**Files**: `tests/integration/security_headers_test.rs` (new),
`tests/integration/mod.rs`

- `#[ignore]` test following the `cors_test.rs` pattern: `GET /api/health`
  against a running server and asserts each of the four headers with its exact
  expected value. Module registered in `mod.rs`.

## WP C — Plan document (LOW)

**Files**: `specs/010-security-headers/plan.md` (this file), `AGENTS.md`

- Plan versioned in the repo following the 007/008/009 convention.
- SPECKIT pointer in `AGENTS.md` updated to `specs/010-security-headers/plan.md`.

## Validation

- `cargo build --locked`, `cargo clippy --workspace --all-targets`,
  `cargo fmt --check`, `cargo test --lib` — all green.
- Manual: `curl -i http://localhost:3000/api/health` confirms all four headers.
- Integration: `cargo test --test integration -- --ignored security_headers`
  against a running server.

## Notes

- Issue #90 auto-closes only when the PR reaches `main` (same as #85/#86/#91),
  so a `development` → `main` promotion is required at the end.
