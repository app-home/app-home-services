# Implementation Plan: Security Hardening (Post-Audit Remediation)

**Branch**: fix/54-security-issues-unwrap-toctou
**Date**: 2026-07-17

## Summary

Remediate the remaining vulnerabilities found during the post-PR security audit.

## WP A — Input Size Limits (HIGH)

**Files**: src/adapters/inbound/login_routes.rs, refresh_routes.rs, oauth_callback.rs

- Add max-length constants (256 for username, 4096 for password, 8192 for refresh_token, 16384 for id_token).
- Add length checks returning 422 after empty checks (with 50ms timing-safe delay).
- Add #[schema(max_length = ...)] to request struct fields.
- Unit tests: at max length OK, over by 1 -> 422, empty -> 422.

## WP B — Atomic rate-limit check-and-record (MEDIUM)

**Files**: src/application/ports/rate_limiter.rs, memory_rate_limiter.rs, redis_rate_limiter.rs, login_routes.rs, refresh_routes.rs

- Add try_check_and_record(&self, ip) -> bool to RateLimiter trait.
- MemoryRateLimiter: hold the Mutex lock for both check + increment.
- RedisRateLimiter: Lua script that INCR + EXPIRE + returns current <= max.
- Replace check+record_attempt sequence with single try_check_and_record AFTER validation.

## WP C — Metrics endpoint & bind address (MEDIUM)

**Files**: src/infrastructure/config/settings.rs, src/main.rs

- Change default server_host from 0.0.0.0 to 127.0.0.1.
- Log a warning at startup when binding to 0.0.0.0 about metrics exposure.

## WP D — LazyLock bcrypt expect (MEDIUM)

**Files**: src/application/use_cases/login_with_password.rs

- Change LazyLock<String> to LazyLock<Option<String>>.
- On hash failure: log error, store None, fall back to thread::sleep(50ms).
- No more process crash on first login.

## WP E — Low-severity items

**5a**. Settings Debug redaction: manual impl fmt::Debug that hides secrets.
**5b**. bcrypt::verify error logging: log tracing::error! before returning false.
**5c**. JWT secret entropy: warn on low-entropy secrets, reject all-same-char.

## Testing

- cargo build — zero warnings
- cargo clippy --all-targets -- -D warnings — zero warnings
- cargo test — all pass

## Order

All WPs touch different files (except login_with_password.rs for WP D + WP E) and are independent.
