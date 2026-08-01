# Implementation Plan: Database Connection TLS (sslmode)

**Branch**: fix/85-database-ssl
**Date**: 2026-07-31
**Issue**: #85

## Summary

`sslmode=disable` in `DATABASE_URL` transmits database credentials and all data
in plaintext over the network. This plan makes plaintext-to-a-remote-database a
fatal startup error, adds an explicit `DB_REQUIRE_SSL=true` switch that forces
`sslmode=verify-full`, and documents the modes.

## WP A — Validation + `DB_REQUIRE_SSL` (HIGH)

**Files**: `crates/shared/src/config/settings.rs`, `crates/shared/Cargo.toml`

- Add `url = "2"` dependency to `shared`.
- Add `Settings::db_require_ssl: bool` (from `DB_REQUIRE_SSL`, truthy = `1`/`true`/`yes`).
- `validate_database_ssl(url)` — fatal `Err` if `sslmode=disable` against a non-loopback host (`127.0.0.1`, `localhost`, `::1` / `IpAddr::is_loopback`).
- `database_ssl_warning(url)` — non-fatal warning if `sslmode` is absent or `prefer` against a non-loopback host.
- `force_sslmode_verify_full(url)` — when `DB_REQUIRE_SSL=true`, rewrite URL so `sslmode=verify-full` (replace/append, preserve other query params).
- Wire all three into `Settings::from_env` (`?` for the fatal check, `eprintln!` for the warning, matching `auth_settings.rs` conventions).

## WP B — Tests

**Files**: `crates/shared/tests/database_ssl_test.rs` (new)

Pure-function integration tests (pattern: `jwt_secret_strength_test.rs`):

- disable + `127.0.0.1`/`localhost`/`::1` → OK; disable + remote hostname/IP → Err.
- `require`/`verify-ca`/`verify-full` + remote → OK; missing/`prefer` + remote → OK but warning.
- Malformed URL → Err.
- `force_sslmode_verify_full`: appends when missing, replaces existing values, preserves other params, forced URL passes validation, malformed URL → Err.

## WP C — Docs & env

**Files**: `.env.example`, `README.md`, `docs/modules/infrastructure.md`, `docs/postgres-ssl.md` (new)

- `.env.example`: document `DB_REQUIRE_SSL` (sslmode modes already documented).
- `docs/postgres-ssl.md`: modes table, non-loopback guardrail, `DB_REQUIRE_SSL`, local-dev notes (mirrors `docs/redis-security.md`).
- Config tables in `README.md` + `docs/modules/infrastructure.md` gain `DB_REQUIRE_SSL`.

## Testing

- `cargo build` — zero warnings
- `cargo clippy --all-targets -- -D warnings` — zero warnings
- `cargo test` — all pass

## Notes

- `.env` is gitignored and uses `sslmode=disable` against `127.0.0.1` (loopback) — still starts fine.
- Applying `DB_REQUIRE_SSL=true` with a self-signed-cert local Postgres will fail to connect (`verify-full`), as intended; it is a production switch.
