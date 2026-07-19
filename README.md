# app-home-services

User authentication service supporting local password login, Google OAuth, session-based JWT authentication, audit trail, rate limiting, and CORS restrictions.

## Requirements

- Rust 2024 edition (nightly)
- PostgreSQL 14+
- Redis (optional, only required for multi-instance deployments -- see Rate Limiting below)

## Setup

1. **Configure environment**

   ```bash
   cp .env.example .env
   # Edit .env with your database URL and secrets
   ```

2. **Create the database**

   ```bash
   createdb app_home
   ```

3. **Run**

   Make sure PostgreSQL is running, then start the service:

   ```bash
   cargo run
   ```

   Migrations are applied automatically on startup (via `sqlx::migrate!`). On first run, the default local user is also seeded. The process aborts with a clear error if the database is unreachable, if the initial default-user check fails, or (when `REDIS_URL` is set) if Redis is unreachable.

## Environment Variables

| Variable | Required | Default | Description |
| ---------- | ---------- | --------- | ------------- |
| `DATABASE_URL` | Yes | — | PostgreSQL connection string |
| `SERVER_HOST` | No | `0.0.0.0` | HTTP server bind host |
| `SERVER_PORT` | No | `3000` | HTTP server bind port |
| `DEFAULT_USER_USERNAME` | No | `admin` | Default local user username |
| `DEFAULT_USER_PASSWORD` | Yes | — | Default local user password |
| `DEFAULT_USER_EMAIL` | No | `admin@example.com` | Default local user email |
| `GOOGLE_CLIENT_ID` | No | — | Google OAuth client ID (empty = Google login disabled) |
| `JWT_SECRET` | Yes | — | HMAC secret for signing JWT tokens (e.g. `openssl rand -hex 64`) |
| `ACCESS_TOKEN_EXPIRY_MINUTES` | No | `15` | Access token lifetime in minutes |
| `REFRESH_TOKEN_EXPIRY_DAYS` | No | `7` | Refresh token lifetime in days |
| `RATE_LIMIT_MAX_ATTEMPTS` | No | `10` | Max failed login attempts per IP within the time window |
| `RATE_LIMIT_WINDOW_SECONDS` | No | `300` | Rate limit window in seconds (default: 5 min) |
| `REDIS_URL` | No | — | Redis URL for shared rate-limit counters; empty = in-memory (single instance only) |
| `CORS_ALLOWED_ORIGINS` | No | — | Comma-separated allowed origins; empty = same-origin only |
| `TRUSTED_PROXY_IPS` | No | — | Comma-separated reverse proxy IPs trusted to set X-Forwarded-For/X-Real-IP; empty = never trusted |

## API Endpoints

### Authentication

| Method | Path | Auth | Description |
| -------- | ------ | ------ | ------------- |
| POST | `/api/auth/login/password` | No | Login with username/password |
| POST | `/api/auth/login/google` | No | Login with Google OAuth ID token |
| POST | `/api/auth/logout` | Bearer | Invalidate a session |
| POST | `/api/auth/refresh` | No | Rotate refresh token for a new access + refresh pair |

### User Profiles

| Method | Path | Auth | Description |
| -------- | ------ | ------ | ------------- |
| GET | `/api/profile` | Bearer | Get the authenticated user's profile |
| PUT | `/api/profile` | Bearer | Update the authenticated user's profile |

### Admin

| Method | Path | Auth | Description |
| -------- | ------ | ------ | ------------- |
| GET | `/api/admin/users` | Bearer+Admin | List all users |
| GET | `/api/admin/users/{id}` | Bearer+Admin | Get a user by ID |
| PUT | `/api/admin/users/{id}/role` | Bearer+Admin | Update a user's role |

### System

| Method | Path | Auth | Description |
| -------- | ------ | ------ | ------------- |
| GET | `/api/health` | No | Health check |
| GET | `/metrics` | No | Prometheus metrics (see Metrics & Alerting below) |

### API Documentation (OpenAPI / Swagger)

The service exposes an auto-generated OpenAPI 3.x specification and an interactive Swagger UI:

| Resource | URL |
| ---------- | ----- |
| Swagger UI | `http://localhost:3000/swagger-ui` |
| OpenAPI JSON | `http://localhost:3000/api-docs/openapi.json` |

The specification is generated from code via `utoipa` and stays in sync with the implementation. All auth endpoints, request/response schemas, status codes, and the Bearer JWT security scheme are documented. Run `cargo test` to validate spec coverage and consistency with the Markdown contracts under `specs/*/contracts/`.

### Login Responses

Successful login returns:

```json
{
  "status": "authenticated",
  "user_id": "uuid",
  "access_token": "jwt...",
  "refresh_token": "jwt..."
}
```

- `access_token`: Short-lived JWT (default 15 min) for authenticating subsequent requests.
- `refresh_token`: Longer-lived JWT (default 7 days) used with `/api/auth/refresh` to obtain a new token pair.

Failed logins return `401` with `{"error": "Invalid username or password"}`. Password verification always performs exactly one bcrypt check (a real one, or a dummy one of equal cost when the username doesn't exist or has no password set), so a nonexistent username can't be told apart from a wrong password by response time; a flat 50 ms delay is layered on top as additional defense-in-depth.

### Using the Auth Middleware

Protected endpoints (like `/api/auth/logout`) require the `Authorization: Bearer <access_token>` header. The server validates the token's signature and expiry, then extracts the `user_id` from its claims.

### Logout

```json
// Request
{ "session_id": "uuid" }

// Response 200
{ "status": "logged_out" }
```

The session is marked inactive (one-way transition). Subsequent refresh attempts with that session's tokens will be rejected.

### Token Refresh

```json
// Request
{ "refresh_token": "jwt..." }

// Response 200
{
  "access_token": "jwt...",
  "refresh_token": "jwt..."
}
```

Each refresh:

1. Validates the old refresh token
2. Verifies the session is active and not expired
3. Invalidates the old session
4. Creates a new session with a new refresh token hash
5. Returns a new access + refresh token pair (token rotation)

### Rate Limiting

Both `/api/auth/login/password` and `/api/auth/refresh` are rate limited per IP address using a sliding window (default: 10 attempts per 5 minutes each). When the limit is exceeded, the endpoint returns `429 Too Many Requests`. A successful login/refresh resets the counter for that IP.

Login and refresh are tracked with **independent counters** (separate `MemoryRateLimiter` instances, or separate Redis key namespaces `ratelimit:login:*` / `ratelimit:refresh:*` when `REDIS_URL` is set) -- exhausting one endpoint's limit for an IP has no effect on the other.

Only requests arriving from an IP listed in `TRUSTED_PROXY_IPS` may use `X-Forwarded-For`/`X-Real-IP` to identify the client; otherwise the real TCP peer address is used, since forwarded headers can be spoofed by any client.

The rate limiter backend is chosen automatically at startup:

- **`REDIS_URL` unset (default):** in-memory counters (`MemoryRateLimiter`). Only safe for a single running instance -- counters are lost on restart and are not shared with other replicas.
- **`REDIS_URL` set:** Redis-backed counters (`RedisRateLimiter`), incremented atomically via a Lua script. Counters are shared across every instance connected to the same Redis, so the limit stays effective when the service is scaled horizontally or restarted. If Redis is temporarily unreachable, the limiter fails open (allows the request) and logs an error, rather than blocking every login/refresh -- this is observable via metrics, see Metrics & Alerting below.

### CORS

Cross-origin requests are restricted to origins listed in `CORS_ALLOWED_ORIGINS` (comma-separated). When the variable is empty, all cross-origin requests are denied (same-origin policy only).

## Architecture

The project is a modular monolith built with Hexagonal Architecture (Ports & Adapters)
and Domain-Driven Design. Each bounded context lives in its own workspace crate:

```text
 ┌─────────────────────────────────────────────────────────────────────────────┐
 │                              src/ (composition root)                        │
 │                        Axum router · combined OpenAPI spec                  │
 └───────┬───────────────┬─────────────────┬──────────────────┬────────────────┘
         │               │                 │                  │
         ▼               ▼                 ▼                  ▼
 ┌───────────────┐ ┌──────────────┐ ┌──────────────┐ ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐
 │   auth/       │ │  profiles/   │ │   admin/     │ │       ???/            │
 │               │ │              │ │              │ │                       │
 │   Domain      │ │   Domain     │ │   Domain     │ │      Domain           │
 │     │         │ │     │        │ │     │        │ │        │              │
 │   UseCases    │ │   UseCases   │ │   UseCases   │ │      UseCases         │
 │     │         │ │     │        │ │     │        │ │        │              │
 │   Ports       │ │   Ports      │ │   Ports      │ │      Ports            │
 │     │         │ │     │        │ │     │        │ │        │              │
 │   Adapters    │ │   Adapters   │ │   Adapters   │ │      Adapters         │
 │               │ │              │ │              │ │                       │
 └───────┬───────┘ └──────┬───────┘ └──────┬───────┘ └─────────┬─────────────┘
         │                │                │                   │
         └────────────────┼────────────────┘  ─  ─ ─ ─ ─ ─ ─ ─ ┘
                          │                    (crates futuros)
                          │
                          ▼
 ┌─────────────────────────────────────────────────────────────────────────────┐
 │                             crates/infrastructure/                          │
 │                     DB pool · telemetry · rate limiter                      │
 └────────────────────────────────┬────────────────────────────────────────────┘
                                  │
                                  ▼
 ┌─────────────────────────────────────────────────────────────────────────────┐
 │                                crates/shared/                               │
 │                           Settings · common utilities                       │
 └─────────────────────────────────────────────────────────────────────────────┘
```

### Project Structure

| Crate | Purpose |
| ----- | ------- |
| `src/` | Composition root — server bootstrap, combined OpenAPI spec |
| `crates/auth/` | Auth context — domain, use cases, inbound/outbound adapters |
| `crates/profiles/` | User profiles context — domain, use cases, adapters |
| `crates/admin/` | Admin user management context — domain, use cases, adapters |
| `crates/infrastructure/` | Shared infrastructure — database pool, telemetry, rate limiter setup |
| `crates/shared/` | Shared types — config settings, common utilities |

### Key Modules (Auth context — `crates/auth/src/`)

| Layer | Path | Description |
| ------- | ------ | ------------- |
| Domain | `domain/entities/` | `User`, `Session`, `UserAction` entities |
| Domain | `domain/aggregate.rs` | `UserAggregate` with domain events & invariant validation |
| Domain | `domain/errors.rs` | `AuthError` enum with typed error variants |
| Application | `application/ports/` | Traits: `UserRepository`, `SessionRepository`, `JwtService`, `RateLimiter`, `AuthProvider` |
| Application | `application/use_cases/` | `login_with_password`, `login_with_google`, `logout`, `refresh_token`, `record_audit_entry` |
| Adapters | `adapters/inbound/` | HTTP handlers + auth middleware |
| Adapters | `adapters/outbound/` | `PostgresUserRepo`, `PostgresSessionRepo`, `JwtServiceImpl`, `MemoryRateLimiter`, `RedisRateLimiter`, `GoogleAuthProvider` |
| Config | `config/` | `AuthSettings` (auth-specific env vars) |

For the other bounded contexts, see:
- `crates/profiles/src/` — Profile entity, `ProfileRepository`, `get_profile` / `update_profile` use cases
- `crates/admin/src/` — `AdminUser` entity, `Role` value object, `AdminRepository`, `list_users` / `get_user` / `update_user_role` use cases

## Migrations

| File | Description |
| ------ | ------------- |
| `001_create_users_table.sql` | Users table with local + Google auth support |
| `002_create_user_actions_table.sql` | Audit trail for auth events |
| `003_create_sessions_table.sql` | Sessions table for JWT refresh token management |
| `004_extend_user_actions.sql` | Adds `session_id` and `event_type` to user_actions |
| `005_add_auth_method_to_sessions.sql` | Adds `auth_method` to sessions (`password` / `google_oauth`) |
| `006_create_user_profiles_table.sql` | User profiles table for the profiles context |
| `007_add_role_to_users.sql` | Adds `role` column (`user` / `admin`) to users table |

Migrations run automatically on startup.

## Testing

```bash
# Run all unit tests (no database or Redis required)
cargo test

# Run integration tests (requires running PostgreSQL + server)
cargo test -- --ignored

# Run Redis integration tests specifically (requires a running Redis)
REDIS_URL=redis://127.0.0.1:6379 cargo test --test integration -- --ignored redis_rate_limit
```

- **Unit tests**: Session entity, JWT service, rate limiter (in-memory), trusted-proxy IP resolution, user action audit, password hashing
- **Integration tests** (ignored by default): Login, logout, refresh, refresh rate limiting, CORS, rate limiting, startup hardening, Redis-backed rate limiting, Redis auth enforcement, live Redis connection failure

### Podman test environment

A helper script at `scripts/test-with-podman.ps1` automates the full test setup:

- Spins up PostgreSQL and Redis via Podman Compose
- Runs unit tests (fast, no external dependencies)
- Builds the server, starts it locally, and waits for the health endpoint
- Runs all integration tests against the live server
- Tears down containers and cleans up

Run it from the project root:

```powershell
.\scripts\test-with-podman.ps1
.\scripts\test-with-podman.ps1 -IntegrationOnly   # skip unit tests
.\scripts\test-with-podman.ps1 -UnitOnly           # no containers, unit tests only
.\scripts\test-with-podman.ps1 -NoTeardown         # keep containers after run
```

See `Get-Help .\scripts\test-with-podman.ps1` for full details.

## Metrics & Alerting

The service exposes a Prometheus-compatible metrics endpoint:

```text
GET /metrics
```

This is not authenticated, so it should only be reachable from inside your monitoring network/namespace, not exposed publicly -- same expectation as any Prometheus scrape target.

### Available metrics

| Metric | Type | Labels | Description |
| -------- | ------ | -------- | ------------- |
| `rate_limiter_redis_errors_total` | Counter | `scope="login"` \| `scope="refresh"` | Cumulative count of Redis errors encountered by the rate limiter (i.e. every time it failed open and allowed a request through instead of enforcing the limit). Absent/zero when running on the in-memory backend (`REDIS_URL` unset), since that backend has no equivalent failure mode. Resets to 0 on process restart. Polled from the rate limiter's internal counter and republished every 15 seconds. |

### Scraping

Add a scrape target in your Prometheus config, e.g.:

```yaml
scrape_configs:
  - job_name: app-home-services
    static_configs:
      - targets: ["app-home-services:3000"]
```

### Alerting

An example alert rule lives in `prometheus/alerts.yml`, firing when `rate_limiter_redis_errors_total` increases at all within a 5-minute window. The threshold starts deliberately low (`> 0`) since there's no baseline yet for what "normal" transient Redis noise looks like in this deployment -- see [`docs/alerting.md`](docs/alerting.md) for the full reasoning and a concrete process for raising the threshold once you have a couple of weeks of real data.

## Security

- Passwords hashed with bcrypt (never stored in plaintext)
- Refresh tokens hashed with bcrypt before storage
- JWT tokens signed with HMAC-SHA256
- No plain-text passwords in logs (structured field logging)
- Rate limiting per IP on both login and refresh (independent counters) to prevent brute-force attacks, backed by Redis for multi-instance deployments (see Rate Limiting above)
- `X-Forwarded-For`/`X-Real-IP` only trusted from configured reverse proxies (`TRUSTED_PROXY_IPS`), preventing rate-limit bypass via header spoofing
- Password login always performs exactly one bcrypt verification (real or a fixed-cost dummy), closing the timing side-channel that would otherwise reveal whether a username exists; a uniform 50 ms delay is layered on top as additional defense-in-depth
- CORS denied by default (same-origin only)
- Startup aborts on database connection failure, default-user seed check failure, or Redis connection failure (when configured)
- Session state transitions are one-way (active → inactive)
- Sessions record the `auth_method` used to create them ("password" / "google_oauth"), so logout/refresh audit entries reflect the real method instead of assuming one
- Redis connections support password auth (`redis://:password@host:port`); TLS is not crate-native today -- see `docs/redis-security.md` for the documented decision and when to revisit it

