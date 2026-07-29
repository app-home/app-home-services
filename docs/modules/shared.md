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

## Networking (`net`)

```rust
pub fn resolve_client_ip(peer_ip: IpAddr, headers: &HeaderMap, trusted_proxies: &[IpAddr]) -> IpAddr
```

Resolves the "real" client IP for a request, honoring `X-Forwarded-For`/`X-Real-IP` only when the direct TCP peer is in `trusted_proxies` -- otherwise any client could spoof these headers. Cross-cutting (used by `auth`'s login/refresh rate limiting and by `infrastructure`'s `/metrics` IP allowlist guard), which is why it lives here rather than being owned by whichever context happened to need it first. Unit-tested directly in this module.

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
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub db_acquire_timeout_seconds: u64,
    pub db_idle_timeout_seconds: u64,
    pub db_max_lifetime_seconds: u64,
    pub metrics_allowed_ips: Vec<IpAddr>,
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

**`UserDirectory`** trait (`user_directory` module):

| Method | Description |
|--------|-------------|
| `get_user_summary(id)` | Look up one user's identity fields |
| `list_user_summaries()` | List every user's identity fields |

Implemented by `auth` (`PostgresUserDirectory`), consumed by `admin` -- see `docs/modules/admin.md`.

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
This clean graph, plus the `AuthenticatedUser` extractor, `EventBus`, and
`UserDirectory` above (all of which let `profiles`/`admin` interact with
cross-cutting or `auth`-originated concerns without depending on the `auth` crate
directly), are what make it plausible to extract a context into its own service
later without a rewrite. See
[`docs/adr/0001-modular-monolith.md`](../adr/0001-modular-monolith.md) for the full
reasoning, including the coupling that *isn't* this clean yet (documented in
`docs/modules/profiles.md`).
