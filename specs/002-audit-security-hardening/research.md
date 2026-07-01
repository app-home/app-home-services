# Research: Audit & Security Hardening

**Date**: 2026-07-01

## Decisions

### JWT Session Tokens
- **Decision**: Use HMAC-SHA256 symmetric signing for JWT tokens
- **Rationale**: Single-service deployment. No need for asymmetric keys. `jsonwebtoken` crate already in use for Google OAuth. Access tokens are short-lived (15 min), refresh tokens (7 days) with rotation.
- **Alternatives considered**: RSA key pair (overkill for single service), opaque session IDs (requires DB lookup on every request), PASETO (new dependency, same security model)

### Session Storage
- **Decision**: Store refresh token hash (bcrypt) in `sessions` table; access tokens are stateless JWTs
- **Rationale**: Refresh tokens need server-side invalidation (logout). Bcrypt hashing prevents DB leak from compromising tokens. Access tokens validated locally by signature (no DB call per request).
- **Alternatives considered**: Storing all tokens in DB (too many DB calls), storing only session ID in JWT (requires DB lookup for every access)

### Rate Limiting
- **Decision**: In-memory sliding window per IP address
- **Rationale**: Single-instance deployment. No external cache (Redis) needed for single-admin scale. Memory-based is simplest and fastest.
- **Alternatives considered**: Redis (overkill for single-admin), fixed window (less precise timing, can allow burst at window boundary), token bucket (more complex)

### CORS
- **Decision**: Deny all cross-origin by default; configurable allow-list via `CORS_ALLOWED_ORIGINS` env var
- **Rationale**: Zero-trust default. Single-admin API likely called from same origin or known client. Configuration via env var allows operator to set without code changes.

### Startup Hardening
- **Decision**: `seed_default_user` returns `Result`; caller uses `expect()` to abort on error
- **Rationale**: Matches existing pattern in `main.rs` (pool creation, migrations all use `expect()`). Consistent and idiomatic.

### Refresh Token Rotation
- **Decision**: Rotate on each use (issue new pair, invalidate old session)
- **Rationale**: Prevents replay attacks if a refresh token is stolen. Standard security practice (as recommended by OAuth 2.0 RFC 6819).
- **Alternatives considered**: Keep same refresh token until expiry (simpler but less secure)
