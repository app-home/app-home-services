# Implementation Plan: Bcrypt cost 12 (OWASP) with configurable override

**Branch**: feat/94-bcrypt-cost
**Date**: 2026-08-04
**Issue**: #94

## Summary

Issue #94 (security, Low) reports `bcrypt::DEFAULT_COST` (10 rounds) is used for
password hashing while OWASP recommends 12-14 on modern hardware. Resolution
(confirmed):

- **Centralized default constant** `DEFAULT_BCRYPT_COST = 12` (in
  `crates/auth/src/config/auth_settings.rs`, co-located with its validation),
  replacing every `bcrypt::DEFAULT_COST` call site.
- **Configurable override** via `BCRYPT_COST` env var, validated fail-fast at
  startup to `12..=31` (floor = OWASP minimum, ceiling = bcrypt max) so an
  operator can tune for hardware without silently weakening security. Default 12.
- **Timing safety preserved**: the precomputed dummy hash used to equalize the
  "user not found" path is now cached per configured cost, so the side-channel
  guard stays effective at any cost.

## WP A — Settings: `BCRYPT_COST` env var (HIGH)

**Files**: `crates/auth/src/config/auth_settings.rs`

- New field `pub bcrypt_cost: u32` in `AuthSettings`.
- Parsed in `from_env` from `BCRYPT_COST` (default `12`), validated with a
  fail-fast `validate_bcrypt_cost` (rejects `< 12` and `> 31`, same
  `Err(String)` startup-error pattern as `validate_jwt_secret`/`#82`).
- `Debug` impl includes the cost (it is not secret).
- Unit tests for the validator: accepts 12/31, rejects 11, rejects 32, rejects
  non-numeric, accepts default.

## WP B — Domain: centralized constant + timing-safe cost threading (HIGH)

**Files**: `crates/auth/src/domain/services/password_verification.rs`

- `pub const DEFAULT_BCRYPT_COST: u32 = 12;` lives in
  `auth_settings.rs` (single source, next to `validate_bcrypt_cost`); the domain
  service imports it for tests.
- `PasswordVerificationService::hash_password(password, cost)` takes the cost.
- `verify_password_timing_safe(user, password, cost)` gains a `cost` parameter.
- The static `DUMMY_PASSWORD_HASH` (fixed cost) is replaced by a
  `LazyLock<Mutex<HashMap<u32, Option<String>>>>` cache keyed by cost, so the
  dummy hash is computed once per cost and `verify` on the not-found path costs
  the same as a real verify. On hash failure it falls back to the existing 50ms
  sleep.
- Callers pass `settings.bcrypt_cost` (see WP C / main.rs).

## WP C — Use cases + seed (HIGH)

**Files**:
`crates/auth/src/application/use_cases/login_with_password.rs`,
`crates/auth/src/application/use_cases/login_with_google.rs`,
`crates/auth/src/application/use_cases/refresh_token.rs`,
`src/main.rs`

- All `bcrypt::hash(..., bcrypt::DEFAULT_COST)` calls (password + refresh-token
  hashing) switch to `settings.bcrypt_cost`. `login_with_password` passes it to
  `verify_password_timing_safe`.
- `seed_default_user` (`src/main.rs`) hashes `default_user_password` with
  `settings.bcrypt_cost`.

## WP D — Tests (MEDIUM)

**Files**: `crates/auth/tests/password_test.rs`, `crates/auth/tests/timing_safety_test.rs`

- `password_test.rs`: hashes at a low test cost (`TEST_BCRYPT_COST = 4`, matching
  the existing `refresh_token_reuse_test`/`session_auth_method_test` pattern);
  verify behavior is cost-independent.
- `timing_safety_test.rs`: passes `DEFAULT_BCRYPT_COST` to
  `verify_password_timing_safe` and hashes the real hash at the same cost so the
  timing comparison stays realistic (it already runs at cost 10 today).

## WP E — Docs + test harness (LOW)

**Files**: `README.md`, `.env.example`, `compose.yaml`

- `BCRYPT_COST` row in the env-var table: default `12`, range `12..=31`,
  OWASP rationale, tuning note.
- `.env.example`: `BCRYPT_COST=` with a comment block.
- `compose.yaml` test-runner is left unchanged: it runs the real process, so the
  fail-fast OWASP floor applies there too (no `BCRYPT_COST=4`). The 53-test
  podman suite stays at the default cost 12; login-heavy tests add roughly
  +0.5s/hash, still well within a normal CI budget. Low costs are only used in
  direct unit tests (`TEST_BCRYPT_COST = 4`), which never go through `from_env`.

## Validation (all green)

- `cargo build --locked`, `cargo clippy --locked --workspace --all-targets`,
  `cargo fmt --all --check` (touched files only; the repo has pre-existing fmt
  churn outside this PR), `cargo test --locked --workspace`.
- `scripts/test-with-podman.ps1 -IntegrationOnly`: 53/53 integration tests on
  the plain-HTTP path at the default cost.
- Startup fail-fast smoke: `BCRYPT_COST=11` → startup aborts before DB pool
  creation; `BCRYPT_COST=12`/unset → default 12.

## Notes

- Refresh-token hashing uses the same bcrypt cost as passwords (the `#94` issue
  and its proposed constant cover every `bcrypt::DEFAULT_COST` call site);
  switching refresh tokens to a faster primitive is out of scope here.
- Issue #94 auto-closes only when the PR reaches `main`, so a
  `development` → `main` promotion is required at the end.
