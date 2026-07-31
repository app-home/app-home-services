# Implementation Plan: Gate Swagger UI behind `ENABLE_SWAGGER`

**Branch**: fix/86-enable-swagger
**Date**: 2026-07-31
**Issue**: #86

## Summary

Swagger UI (`/swagger-ui`) and the full OpenAPI spec (`/api-docs/openapi.json`)
are served unauthenticated, letting anyone enumerate the complete API surface.
This plan gates both behind an explicit `ENABLE_SWAGGER=true` flag, **disabled by
default**, so a publicly reachable instance registers neither route (both return
`404`).

## WP A — `ENABLE_SWAGGER` setting (HIGH)

**File**: `crates/shared/src/config/settings.rs`

- Add `Settings::enable_swagger: bool` (from `ENABLE_SWAGGER`, truthy = `1`/`true`/`yes`,
  default `false`), reusing the exact parsing pattern of `DB_REQUIRE_SSL`.
- Add `enable_swagger` to the manual `Debug` impl.

## WP B — Conditional router mount (HIGH)

**File**: `src/main.rs`

- Build the main router as `let mut app = ...`; only when
  `settings.enable_swagger` is `true`, `app = app.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))`.
- Add a startup log line for both branches (mirrors the `METRICS_ALLOWED_IPS`
  pattern): "serving Swagger UI..." vs "docs disabled (no API surface exposure)".
- `ApiDoc::openapi()` is a compile-time generated static spec, so the conditional
  has no runtime cost.

## WP C — Tests

**Files**: `crates/infrastructure/src/rate_limiter_setup.rs`, `tests/openapi_spec_served.rs`

- Add `enable_swagger: false` to the `Settings` literal in the
  `settings_with_redis_url` test helper (any struct literal of `Settings` breaks
  otherwise).
- `tests/openapi_spec_served.rs` (ignored, hits a running server): document in the
  header that the server must be started with `ENABLE_SWAGGER=true` for the 200
  assertions to hold.
- No new unit tests for parsing: `from_env` reads process-global env (racy in
  parallel tests) and the bool parsing is a single `matches!` — same rationale as
  `DB_REQUIRE_SSL`.

## WP D — Docs & env (MEDIUM)

**Files**: `.env.example`, `README.md`, `docs/modules/infrastructure.md`, `docs/modules/shared.md`

- `.env.example`: new `ENABLE_SWAGGER=false` section explaining the security
  rationale (issue #86) and the local-dev (`true`) vs production (unset) guidance.
- `README.md`: add the env table row and rewrite the "API Documentation" section to
  state the routes only exist when `ENABLE_SWAGGER=true`.
- Config tables gain `ENABLE_SWAGGER` in `docs/modules/infrastructure.md`; the
  `Settings` struct listing in `docs/modules/shared.md` gains `enable_swagger`
  (and the previously-missing `db_require_ssl`).
- `specs/003-openapi-docs/contracts/openapi-doc.md` + `maintenance.md`: note that
  the endpoints are only mounted when `ENABLE_SWAGGER=true`.

## Testing

- `cargo build` — zero warnings
- `cargo clippy --all-targets -- -D warnings` — zero warnings
- `cargo fmt --check` — clean
- `cargo test` — all pass
- Smoke: run without the flag → `/swagger-ui` and `/api-docs/openapi.json` return
  `404`; run with `ENABLE_SWAGGER=true` → both return `200`.

## Notes

- The default is deliberately **insecure-by-default → secure-by-default**: local
  developers opt in explicitly; nothing to forget in production.
- `compose.yaml` / GitHub workflows set no env for the service image, so the
  published container ships with docs off.
- Issue #86 auto-closes only when the PR reaches the default branch (`main`);
  merging to `development` alone will not close it (same as #85).
