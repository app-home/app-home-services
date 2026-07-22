# Module: Shared (`crates/shared/`)

## Purpose

Shared kernel and cross-cutting types consumed by all bounded contexts. This is the **leaf dependency** in the dependency graph — it depends on no other crate within the workspace.

## Dependencies

None (only external ecosystem crates: `serde`, `thiserror`, `chrono`, `uuid`, `utoipa`, `axum`, `jsonwebtoken`, `async-trait`, `tokio`).

## Domain Layer

### Events

**`Event`** enum — durable audit events consumed by `AuditEventHandler`:

| Variant | Fields |
|---------|--------|
| `UserLoggedIn` | `user_id`, `email`, `auth_method`, `timestamp` |
| `UserLoggedOut` | `user_id`, `session_id`, `timestamp` |
| `SessionRefreshed` | `user_id`, `old_session_id`, `new_session_id`, `timestamp` |
| `UserCreated` | `user_id`, `email`, `auth_provider`, `timestamp` |

**`EventBus`** — `tokio::sync::broadcast` channel:
- `new(capacity)` → `(EventBus, Receiver<Event>)`
- `publish(event)` — non-blocking send
- `subscribe()` → `Receiver<Event>`

### Value Objects

| Value Object | Description |
|-------------|-------------|
| `Email` | Email address string |
| `HashedPassword` | Bcrypt hashed password |
| `AuthProvider` | `"local"` / `"google"` |
| `AuthMethod` | `"password"` / `"google_oauth"` |
| `AccessToken` | JWT access token (newtype) |
| `RefreshToken` | JWT refresh token (newtype) |
| `TokenPair` | `(AccessToken, RefreshToken)` bundle |
| `EventType` | String-based event type classifier |

### Domain Errors

`DomainError`: `InvalidEmail`, `InvalidValue`, `InternalError(String)`

## API Types

Shared request/response types for OpenAPI:

| Struct | Fields |
|--------|--------|
| `ErrorResponse` | `error: String` — standard error envelope |
| `HealthResponse` | `status: String`, `version: String` — `GET /api/health` |

## Auth Extraction

**`AuthenticatedUser`** — JWT Bearer extractor (`FromRequestParts`):
1. Extracts `Authorization: Bearer <token>` header
2. Decodes via `DecodingKey` from `Extension<Arc<DecodingKey>>`
3. Returns `AuthenticatedUser { user_id: Uuid }` or `AuthRejection` (401)

Used by profiles, admin, and auth/logout handlers.

## Configuration

**`Settings`** — all infra-level config:

```rust
pub struct Settings {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub rate_limit_max_attempts: u32,
    pub rate_limit_window_seconds: u64,
    pub cors_allowed_origins: String,
    pub trusted_proxy_ips: Vec<IpAddr>,
    pub redis_url: Option<String>,
}
```

Loaded via `Settings::from_env()`. Debug impl redacts credentials.

## Ports

**`RateLimiter`** trait:

| Method | Description |
|--------|-------------|
| `check(ip)` | Whether request is allowed |
| `record_attempt(ip)` | Record an attempt |
| `try_check_and_record(ip)` | Atomic test+set |
| `remaining_attempts(ip)` | Remaining attempts in window |
| `reset(ip)` | Reset counter (on successful login) |

Implemented by `MemoryRateLimiter` and `RedisRateLimiter` (in `infrastructure`).

## Dependency Graph

```
shared (leaf)
 ↑
 ├── auth
 ├── infrastructure
 ├── profiles
 └── admin
```

No bounded context depends on another — `shared` is the shared kernel at the base.
This clean graph, plus the `AuthenticatedUser` extractor and `EventBus` above (both
of which let `profiles`/`admin` interact with cross-cutting or `auth`-originated
concerns without depending on the `auth` crate directly), are what make it
plausible to extract a context into its own service later without a rewrite. See
[`docs/adr/0001-modular-monolith.md`](../adr/0001-modular-monolith.md) for the full
reasoning, including the coupling that *isn't* this clean yet (documented in
`docs/modules/admin.md` and `docs/modules/profiles.md`).
