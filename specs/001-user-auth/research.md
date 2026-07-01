# Research: User Authentication

## Decisions

### Password Hashing
- **Decision**: Use bcrypt for password hashing.
- **Rationale**: Industry standard for password storage; built-in salt; configurable cost factor; widely vetted.
- **Alternatives considered**: Argon2id (stronger but less library support in Rust ecosystem), SHA-256 (rejected — not designed for password storage).

### Google OAuth Token Verification
- **Decision**: Verify Google ID tokens using OpenID Connect (JWT verification with Google's public keys).
- **Rationale**: Stateless verification; no additional HTTP call to Google on every login after key caching; standard OIDC flow.
- **Alternatives considered**: Google Token Info endpoint (adds latency per request), custom OAuth flow (unnecessary complexity).

### UUID Generation
- **Decision**: Use UUIDv7 (time-ordered) as primary keys.
- **Rationale**: Time-sortable, clustered index-friendly in PostgreSQL, avoids fragmentation of auto-increment gaps, globally unique.
- **Alternatives considered**: UUIDv4 (random, causes B-tree index fragmentation), auto-increment integers (rejected by constitution).

### Session Management
- **Decision**: Out of scope for this feature (handled separately per spec).
- **Rationale**: Authentication and session management are distinct concerns; spec explicitly excludes session handling.
- **Alternatives considered**: JWT issuance on login (deferred), cookie-based sessions (deferred).

### Rust Dependencies
- **Decision**: 
  - `bcrypt` crate for password hashing
  - `openidconnect` or manual `jsonwebtoken` + `reqwest` for Google token verification
  - `uuid` crate with `v7` feature for UUID generation
  - `sqlx` with `postgres` and `uuid` features
  - `tracing` + `tracing-subscriber` for logging
- **Rationale**: All crates are well-maintained, widely used in the Rust ecosystem, and align with existing project dependencies (Tokio, Axum, SQLx).
- **Alternatives considered**: Custom JWT verification (unnecessary — existing libraries are battle-tested).

### Database Migrations
- **Decision**: Use SQLx migration system (`sqlx migrate`).
- **Rationale**: Already mandated by constitution; integrates natively with SQLx; supports `generate`, `run`, and `revert`.
- **Alternatives considered**: Diesel migrations (different ORM), custom migration scripts (non-standard).

## Architecture Notes

### Port/Adapter Flow

```
HTTP Request → Login Routes (inbound adapter)
  → LoginWithPassword / LoginWithGoogle (application use case)
    → UserRepository port → PostgresUserRepo (outbound adapter)
    → AuthProvider port → GoogleAuthProvider (outbound adapter)
  → RecordAuditEntry (application use case)
    → UserActionRepository port → PostgresUserActionRepo (outbound adapter)
  → Response back to HTTP
```

### Authentication Flow Details

1. **Username/Password**: Receive credentials → hash incoming password → compare with stored hash → if match, create audit entry → return success token.
2. **Google OAuth**: Receive ID token → verify JWT signature/iss/aud → extract email → find or create user → create audit entry → return success token.
3. **Error handling**: Generic error messages for all failures; never reveal whether username exists or reason for failure beyond what is safe.

## Open Questions

None — all decisions resolved through spec, constitution, and documented assumptions.
