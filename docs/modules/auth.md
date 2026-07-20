# Module: Auth (`crates/auth/`)

## Purpose

Authentication and session management bounded context. Handles user login (password and Google OAuth), logout, JWT token refresh, and domain event publishing via `EventBus`. Implements DDD with a `UserAggregate` that enforces invariants across `User`, `Session`, and pending domain events.

## Dependencies

| Crate | Role |
|-------|------|
| `shared` | Domain events, value objects (`Email`, `HashedPassword`, `AuthMethod`), `EventBus`, `RateLimiter` trait, `AuthenticatedUser` extractor |

## Domain Layer

### Entities

| Entity | Description |
|--------|-------------|
| `User` | Core user: `id: Uuid`, `username: Option<String>`, `email: Email`, `display_name: String`, `password_hash: Option<HashedPassword>`, `auth_provider: AuthProvider`, timestamps |
| `Session` | Active user session: `id`, `user_id`, `refresh_token_hash`, `expires_at`, `auth_method`, `is_active` |
| `UserAction` | Audit log entry for security-relevant operations |

### Aggregate — `UserAggregate`

Aggregate root containing `User + Vec<Session> + pending domain events`.

| Method | Description |
|--------|-------------|
| `new(user, sessions)` | Creates aggregate from loaded data |
| `add_session(id, hash, expiry, method)` | Enforces `MAX_ACTIVE_SESSIONS = 25`; emits `UserLoggedIn` |
| `invalidate_session(session_id)` | Marks session inactive; emits `UserLoggedOut` |
| `rotate_session(old_id, new_id, hash, expiry)` | Session key rotation; emits `SessionRefreshed` |
| `validate_invariants()` | Validates entity consistency rules |

### Domain Service

`PasswordVerificationService` — bcrypt-based password verification against `HashedPassword`.

### Domain Errors

`AuthError` enum:

- `InvalidCredentials` — `UserNotFound` — `TokenVerificationFailed`
- `SessionNotFound` — `SessionExpired` — `SessionInvalidated`
- `InvalidRefreshToken` — `RateLimited` — `TokenGenerationFailed`
- `InternalError(String)`

## Application Layer (Use Cases)

### Ports (Traits)

| Port | Key Methods |
|------|-------------|
| `UserRepository` | `find_by_username`, `find_by_email`, `find_by_id`, `create`, `create_user_action`, `find_aggregate_by_*`, `save_aggregate` |
| `SessionRepository` | `create`, `find_by_id`, `find_active_by_user_id`, `invalidate`, `invalidate_all_for_user`, `sessions_for_user` |
| `JwtService` | `generate_token_pair(user_id, session_id)` → `TokenPair`, `validate_access_token`, `validate_refresh_token` |
| `AuthProvider` | `verify_id_token(token)` → `GoogleUserInfo` (Google OAuth) |

### Use Cases

| Use Case | Input | Output |
|----------|-------|--------|
| `login_with_password` | `username`, `password` | aggregate + tokens + events |
| `login_with_google` | Google `id_token` | aggregate + tokens + `is_new_user` + events |
| `logout` | `user_id`, `session_id` | `Vec<Event>` |
| `refresh_token` | `refresh_token` | `user_id` + new tokens + events |
| `record_audit_entry` | event details | persisted `UserAction` |

## Adapter Layer

### Inbound (HTTP Handlers)

| Method | Path | Auth | Request | Response 200 | Errors |
|--------|------|------|---------|--------------|--------|
| `POST` | `/api/auth/login/password` | None | `PasswordLoginRequest {username, password}` | `AuthTokensResponse` | 401, 422, 429, 500 |
| `POST` | `/api/auth/login/google` | None | `GoogleLoginRequest {id_token}` | `GoogleAuthResponse` | 401, 422, 429, 500 |
| `POST` | `/api/auth/logout` | Bearer JWT | `LogoutRequest {session_id}` | `StatusResponse` | 400, 401, 500 |
| `POST` | `/api/auth/refresh` | None | `RefreshTokenRequest {refresh_token}` | `RefreshResponse` | 401, 422, 429, 500 |

**Proxy-aware IP resolution**: login and refresh handlers use `resolve_client_ip()` which honours `X-Forwarded-For` / `X-Real-IP` from trusted proxies (`TRUSTED_PROXY_IPS`).

**Contracts**: [login-password](../../specs/002-audit-security-hardening/contracts/login-password.md) · [login-google](../../specs/002-audit-security-hardening/contracts/login-google.md) · [logout](../../specs/002-audit-security-hardening/contracts/logout.md) · [refresh](../../specs/002-audit-security-hardening/contracts/refresh.md)

### Outbound (Concrete Implementations)

| Adapter | Implements | Description |
|---------|-----------|-------------|
| `PostgresUserRepo` | `UserRepository` | PostgreSQL user & user_action persistence |
| `PostgresSessionRepo` | `SessionRepository` | Session CRUD with active/inactive tracking |
| `JwtServiceImpl` | `JwtService` | JWT token generation with configurable expiry |
| `GoogleAuthProvider` | `AuthProvider` | Google ID token verification |
| `AuditEventHandler` | — | Consumes `EventBus` events, persists to `user_actions` |
| `MemoryRateLimiter` | `RateLimiter` | In-memory, single-instance safe |
| `RedisRateLimiter` | `RateLimiter` | Redis-backed, multi-instance, fail-open |

### AppState

Wired in `main.rs`, holds all dependencies:

```
user_repo: PostgresUserRepo
session_repo: PostgresSessionRepo
auth_provider: GoogleAuthProvider
jwt_service: JwtServiceImpl
rate_limiter: Arc<dyn RateLimiter>
refresh_rate_limiter: Arc<dyn RateLimiter>
event_bus: EventBus
auth_settings: AuthSettings
trusted_proxy_ips: Vec<IpAddr>
```

## Configuration

`AuthSettings` — `AuthSettings::from_env()`:

| Env Var | Default | Description |
|---------|---------|-------------|
| `DEFAULT_USER_USERNAME` | `admin` | Seed user (only on empty DB) |
| `DEFAULT_USER_PASSWORD` | **required** | Seed user password (bcrypt-hashed) |
| `DEFAULT_USER_EMAIL` | `admin@example.com` | Seed user email |
| `GOOGLE_CLIENT_ID` | (empty) | Google OAuth client ID |
| `JWT_SECRET` | **required** | Min 32 bytes; entropy validated |
| `ACCESS_TOKEN_EXPIRY_MINUTES` | `15` | Access token TTL |
| `REFRESH_TOKEN_EXPIRY_DAYS` | `7` | Refresh token TTL |

## Integration

In `src/main.rs`:

1. `PostgresUserRepo`, `PostgresSessionRepo` created from pool
2. `JwtServiceImpl`, `GoogleAuthProvider` from auth settings
3. Rate limiters via `build_rate_limiters()` (Redis or memory)
4. `AppState::new(...)` bundles all dependencies
5. Routes: `POST /api/auth/login/password`, `POST /api/auth/login/google`, `POST /api/auth/logout`, `POST /api/auth/refresh`
6. `EventBus` receiver → `AuditEventHandler` (background task)