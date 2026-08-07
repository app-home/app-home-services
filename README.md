# app-home-services

User authentication service supporting local password login, Google OAuth, session-based JWT authentication, audit trail, rate limiting, and CORS restrictions.

## Requirements

- Rust 2024 edition (nightly)
- PostgreSQL 14+
- Redis (optional, only required for multi-instance deployments -- see Rate Limiting below)
- Podman or Docker (optional, only for building/running the container image -- see Container Image below)

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
| `DATABASE_URL` | Yes | — | PostgreSQL connection string. See the TLS notes in `.env.example`; `sslmode=disable` against a non-loopback host aborts startup (see `docs/postgres-ssl.md`). |
| `DB_MAX_CONNECTIONS` | No | `10` | Max connections this instance's pool will open. With N instances against one Postgres, they together open up to `N * DB_MAX_CONNECTIONS` -- keep that under Postgres's own `max_connections`. See Database Connection Pool below. |
| `DB_MIN_CONNECTIONS` | No | `0` | Idle connections the pool tries to keep pre-warmed. `0` = open lazily on demand. |
| `DB_ACQUIRE_TIMEOUT_SECONDS` | No | `30` | How long a request waits for a pool connection before failing with a clear timeout instead of hanging. |
| `DB_IDLE_TIMEOUT_SECONDS` | No | `600` | How long a connection may sit idle before being closed. `0` disables idle recycling. |
| `DB_MAX_LIFETIME_SECONDS` | No | `1800` | Max lifetime of a connection before forced recycling, guarding against silently-stale connections behind a proxy/load balancer. `0` disables this. |
| `DB_REQUIRE_SSL` | No | `false` | Force the database connection to use `sslmode=verify-full`, replacing any `sslmode` in `DATABASE_URL`. For environments that demand an encrypted, certificate-verified Postgres connection (e.g. production). |
| `SERVER_HOST` | No | `127.0.0.1` | HTTP server bind host. **Set to `0.0.0.0` when running in a container** (see Container Image below) or anywhere else the process needs to accept connections from outside its own host -- `127.0.0.1` only accepts local connections. |
| `SERVER_PORT` | No | `3000` | HTTP server bind port |
| `TLS_CERT_PATH` | No | — | Path to a PEM-encoded TLS certificate chain. Must be set **together with** `TLS_KEY_PATH`; setting only one aborts startup. When both are set the service terminates HTTPS itself (native TLS via rustls, see HTTPS / TLS below). Empty = plain HTTP. |
| `TLS_KEY_PATH` | No | — | Path to the PEM-encoded private key matching `TLS_CERT_PATH`. Must be set **together with** `TLS_CERT_PATH`; setting only one aborts startup. |
| `DEFAULT_USER_USERNAME` | No | `admin` | Default local user username |
| `DEFAULT_USER_PASSWORD` | Yes | — | Default local user password. Must be at least 12 characters, contain at least 3 of {lowercase, uppercase, digits, symbols}, and not be a known weak/placeholder password (e.g. `admin123`) -- the service refuses to start otherwise. Under 16 characters is accepted but logged as a startup warning. |
| `DEFAULT_USER_EMAIL` | No | `admin@example.com` | Default local user email |
| `GOOGLE_CLIENT_ID` | No | — | Google OAuth client ID (empty = Google login disabled) |
| `JWT_SECRET` | Yes | — | HMAC secret for signing JWT tokens. Must be at least 32 bytes **and** have at least 8 unique characters (rejects both short and low-entropy secrets, e.g. `aaaa...aaaa`) -- generate one with `openssl rand -hex 64` |
| `ACCESS_TOKEN_EXPIRY_MINUTES` | No | `15` | Access token lifetime in minutes |
| `REFRESH_TOKEN_EXPIRY_DAYS` | No | `7` | Refresh token lifetime in days |
| `BCRYPT_COST` | No | `12` | bcrypt cost used for password and refresh-token hashing (OWASP minimum). Must be `>= 12` (OWASP) and `<= 31` (bcrypt's maximum); the service refuses to start otherwise (see #94) |
| `JWT_ISSUER` | No | `app-home-services` | `iss` claim minted/required on tokens; set a distinct value per environment so tokens can't be replayed across environments (see #87) |
| `JWT_AUDIENCE` | No | `app-home-services` | `aud` claim minted/required on tokens; same cross-environment replay rationale as `JWT_ISSUER` |
| `RATE_LIMIT_MAX_ATTEMPTS` | No | `10` | Max failed login attempts per IP within the time window |
| `RATE_LIMIT_WINDOW_SECONDS` | No | `300` | Rate limit window in seconds (default: 5 min) |
| `REDIS_URL` | No | — | Redis URL for shared rate-limit counters and the access-token revocation list; empty = in-memory (single instance only) |
| `REVOCATION_FLUSH_INTERVAL_SECONDS` | No | `5` | How often (seconds) the durable-revocation flush worker retries journaled revocations against Redis (see #140); only meaningful when `REDIS_URL` is set |
| `CORS_ALLOWED_ORIGINS` | No | — | Comma-separated allowed origins; empty = same-origin only |
| `TRUSTED_PROXY_IPS` | No | — | Comma-separated reverse proxy IPs trusted to set X-Forwarded-For/X-Real-IP; empty = never trusted |
| `METRICS_ALLOWED_IPS` | No | — | Comma-separated IPs allowed to reach `GET /metrics` (e.g. your Prometheus server); empty = no restriction. Loopback is always allowed regardless. See Metrics & Alerting below. |
| `ENABLE_SWAGGER` | No | `false` | Serve Swagger UI and the OpenAPI spec at `/swagger-ui` and `/api-docs/openapi.json`. Disabled by default so a publicly reachable instance exposes no API surface; set to `true` for local development. See API Documentation below. |

## HTTPS / TLS

This service expects to be served over HTTPS in production. Two mutually exclusive options exist:

1. **Reverse proxy (default)** — deploy behind a TLS-terminating reverse proxy (Caddy, nginx, Traefik, a cloud load balancer, ...). The service binds plain HTTP on `SERVER_HOST:SERVER_PORT` and the proxy forwards to it. Configure `TRUSTED_PROXY_IPS` so client IP resolution and rate limiting use the real peer address.
2. **Native TLS** — set both `TLS_CERT_PATH` and `TLS_KEY_PATH` (PEM files). The service then terminates HTTPS itself via rustls and you can point clients at it directly; no reverse proxy is needed for encryption. Only one of the two paths is a startup error, so a half-configured deployment can never silently serve plaintext while the operator believes HTTPS is on.

The `Strict-Transport-Security` (HSTS) header is sent on every response (see #90). Browsers only honor it when received over a real HTTPS connection, regardless of whether TLS terminates at this service or at the reverse proxy, so it is harmless in the reverse-proxy plain-HTTP setup (the proxy forwards the header to the client over HTTPS, where it takes effect) and effective in the native-TLS one. If you terminate TLS with your own certificate (e.g. a self-signed one for testing), clients must trust it explicitly or use `curl -k` / equivalent.

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
| GET | `/api/health` | No | Health check -- runs `SELECT 1` against the database pool (2s timeout); `200` if it succeeds, `503` if the database is unreachable or the check times out |
| GET | `/metrics` | No (optionally IP-restricted) | Prometheus metrics; no credentials required, but reachability can be restricted to an IP allowlist via `METRICS_ALLOWED_IPS` (see Metrics & Alerting below) |

### API Documentation (OpenAPI / Swagger)

The service exposes an auto-generated OpenAPI 3.x specification and an interactive Swagger UI **only when `ENABLE_SWAGGER=true`** (default: disabled). Without the flag, both routes return `404`, so a publicly reachable instance does not leak its API surface. For local development, start with `ENABLE_SWAGGER=true`:

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

- `access_token`: Short-lived JWT (default 15 min) for authenticating subsequent requests. Each token carries a unique `jti` so it can be revoked individually (see Logout below).
- `refresh_token`: Longer-lived JWT (default 7 days) used with `/api/auth/refresh` to obtain a new token pair.

Failed logins return `401` with `{"error": "Invalid username or password"}`. Password verification always performs exactly one bcrypt check (a real one, or a dummy one of equal cost when the username doesn't exist or has no password set), so a nonexistent username can't be told apart from a wrong password by response time; a flat 50 ms delay is layered on top as additional defense-in-depth.

### Using the Auth Middleware

Protected endpoints (like `/api/auth/logout`) require the `Authorization: Bearer <access_token>` header. The server validates the token's signature, expiry, `iss`/`aud`, then checks the token's `jti` against the access-token revocation list before extracting the `user_id` from its claims.

### Logout

```json
// Request
{ "session_id": "uuid" }

// Response 200
{ "status": "logged_out" }
```

The session is marked inactive (one-way transition). Subsequent refresh attempts with that session's tokens will be rejected. The presented access token is additionally revoked (by its `jti`) for the rest of its lifetime, so a stolen access token stops validating as soon as the victim logs out (see #88).

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
- **`REDIS_URL` set:** Redis-backed counters (`RedisRateLimiter`), incremented atomically via a Lua script. Counters are shared across every instance connected to the same Redis, so the limit stays effective when the service is scaled horizontally or restarted. If Redis is temporarily unreachable, the limiter falls back to an in-memory per-instance budget (each instance still enforces its own attempt limit; cross-instance coordination is lost until Redis recovers) and logs an error -- observable via metrics, see Metrics & Alerting below.

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
| `crates/infrastructure/` | Shared infrastructure — database pool, telemetry, rate limiter setup, `/metrics` IP allowlist guard |
| `crates/shared/` | Shared types — config settings, common utilities, cross-context ports (`UserDirectory`, `RateLimiter`), event bus, client IP resolution (`net`) |

### Why a modular monolith, and how to extract a context later

Each bounded context is a separate workspace crate today, deployed as one binary,
rather than a separate service — see [`docs/adr/0001-modular-monolith.md`](docs/adr/0001-modular-monolith.md)
for the full reasoning. In short: at the current scale, one deploy is cheaper to
operate than several, and the crate boundaries already give most of the isolation
benefit of a service boundary without the network/versioning cost. That same
document also gives a verified account of exactly what would need to change before
a given context could be cleanly pulled out into its own service (including how
`admin`'s former direct coupling to `auth`'s `users` table was already resolved via
the `UserDirectory` port), and concrete signals for when that becomes worth doing.

### Key Modules (Auth context — `crates/auth/src/`)

| Layer | Path | Description |
| ------- | ------ | ------------- |
| Domain | `domain/entities/` | `User`, `Session`, `UserAction` entities |
| Domain | `domain/aggregate.rs` | `UserAggregate` with domain events & invariant validation |
| Domain | `domain/errors.rs` | `AuthError` enum with typed error variants |
| Application | `application/ports/` | Traits: `UserRepository`, `SessionRepository`, `JwtService`, `RateLimiter`, `AuthProvider` |
| Application | `application/use_cases/` | `login_with_password`, `login_with_google`, `logout`, `refresh_token`, `record_audit_entry` |
| Adapters | `adapters/inbound/` | HTTP handlers + auth middleware |
| Adapters | `adapters/outbound/` | `PostgresUserRepo`, `PostgresUserDirectory`, `PostgresSessionRepo`, `JwtServiceImpl`, `MemoryRateLimiter`, `RedisRateLimiter`, `GoogleAuthProvider` |
| Config | `config/` | `AuthSettings` (auth-specific env vars) |

Client IP resolution (`resolve_client_ip`, used by login/refresh rate limiting and by the `/metrics` IP allowlist guard) lives in `crates/shared/src/net.rs`, not in `auth`, since more than one bounded context needs it.

For the other bounded contexts, see:
- `crates/profiles/src/` — Profile entity, `ProfileRepository`, `get_profile` / `update_profile` use cases
- `crates/admin/src/` — `AdminUser` entity, `Role` value object, `AdminRepository` (backed by its own `user_roles` table plus `shared::user_directory::UserDirectory` for identity fields), `list_users` / `get_user` / `update_user_role` use cases

## Migrations

| File | Description |
| ------ | ------------- |
| `001_create_users_table.sql` | Users table with local + Google auth support |
| `002_create_user_actions_table.sql` | Audit trail for auth events |
| `003_create_sessions_table.sql` | Sessions table for JWT refresh token management |
| `004_extend_user_actions.sql` | Adds `session_id` and `event_type` to user_actions |
| `005_add_auth_method_to_sessions.sql` | Adds `auth_method` to sessions (`password` / `google_oauth`) |
| `006_create_user_profiles_table.sql` | User profiles table for the profiles context |
| `007_add_role_to_users.sql` | Adds `role` column (`user` / `admin`) to users table (superseded by 008) |
| `008_create_admin_user_roles.sql` | Moves `role` into its own `user_roles` table owned by the admin context; drops `users.role` |
| `009_create_access_token_revocation_outbox.sql` | Durable-retry outbox for access token revocations Redis rejects (see #140) |
| `010_access_token_revocation_outbox_expires_at.sql` | Adds the indexed `expires_at` column the outbox expiry sweep uses (`timestamptz + interval` can't be indexed directly) |
| `011_access_token_revocation_outbox_expires_at_idx.sql` | Creates the `expires_at` index concurrently (its own `-- no-transaction` migration, since `CREATE INDEX CONCURRENTLY` can't run inside a transaction) |
| `012_access_token_revocation_outbox_expires_at_repair.sql` | Self-healing guard: drops invalid `_ccnew`/`_ccold` leftovers and rebuilds the `expires_at` index (only if it's invalid) when migration 011's concurrent build left it broken |

Migrations run automatically on startup.

## Database Connection Pool

A single `PgPool` is created once at startup (`infrastructure::database::create_pool`) and shared -- via cheap clones, since `PgPool` wraps an internal `Arc` -- across every bounded context's repositories. It's tuned via the `DB_*` environment variables documented above (`DB_MAX_CONNECTIONS`, `DB_MIN_CONNECTIONS`, `DB_ACQUIRE_TIMEOUT_SECONDS`, `DB_IDLE_TIMEOUT_SECONDS`, `DB_MAX_LIFETIME_SECONDS`), all optional with sensible defaults if unset.

**Sizing for more than one instance:** each instance opens its own pool, so N instances against one Postgres can open up to `N * DB_MAX_CONNECTIONS` connections in total. Make sure that stays comfortably under Postgres's own `max_connections` (commonly `100` by default) -- e.g. 5 instances at the default `DB_MAX_CONNECTIONS=10` is 50 connections, leaving room for `psql`, migrations, and anything else touching the same database.

**`DB_ACQUIRE_TIMEOUT_SECONDS`** bounds how long a request waits for a pool connection when the pool is fully checked out, turning exhaustion into a fast, explicit error rather than a hung request. **`DB_IDLE_TIMEOUT_SECONDS`** and **`DB_MAX_LIFETIME_SECONDS`** recycle connections proactively -- useful if there's a proxy, load balancer, or managed Postgres provider between the app and the database that can silently drop long-lived idle connections.

`/api/health` (see API Endpoints above) exercises this same pool with a real `SELECT 1` query, so it reflects actual database reachability rather than just "the process is running." `db_pool_size` and `db_pool_idle` (see Metrics & Alerting below) give ongoing visibility into pool utilization -- `db_pool_size - db_pool_idle` is the number of connections currently checked out, which approaching `DB_MAX_CONNECTIONS` is the signal for pool exhaustion (see #100).

## Testing

```bash
# Run all unit tests (no database or Redis required)
cargo test

# Run integration tests (requires running PostgreSQL + server)
cargo test -- --ignored

# Run Redis integration tests specifically (requires a running Redis)
REDIS_URL=redis://127.0.0.1:16379 cargo test -- --ignored --test-threads=1 redis_rate_limit
```

- **Unit tests**: Session entity, JWT service, rate limiter (in-memory), client IP resolution, `/metrics` IP allowlist decision logic, user action audit, password hashing, default admin password strength
- **Integration tests** (ignored by default): Login, logout, refresh, refresh rate limiting, CORS, rate limiting, startup hardening, Redis-backed rate limiting, Redis auth enforcement, live Redis connection failure, DB-backed health check, `/metrics` reachability

### Podman test environment

A helper script at `scripts/test-with-podman.ps1` automates the full test setup:

- Spins up PostgreSQL and Redis via Podman Compose
- Runs unit tests (fast, no external dependencies)
- Builds the server, starts it locally, and waits for the health endpoint
- Runs all integration tests against the live server
- Tears down containers and cleans up

`compose.yaml` is only the PostgreSQL + Redis test fixture used by this script; the
test runner itself runs on the host (see #131).

Run it from the project root:

```powershell
.\scripts\test-with-podman.ps1
.\scripts\test-with-podman.ps1 -IntegrationOnly   # skip unit tests
.\scripts\test-with-podman.ps1 -UnitOnly           # no containers, unit tests only
.\scripts\test-with-podman.ps1 -NoTeardown         # keep containers after run
```

See `Get-Help .\scripts\test-with-podman.ps1` for full details.

## Container Image

A `Containerfile` (Docker/Podman-compatible multi-stage build) is included, and every push to `main` automatically builds and publishes an image to GitHub Container Registry via `.github/workflows/docker-publish.yml`:

```
ghcr.io/app-home/app-home-services:latest
ghcr.io/app-home/app-home-services:<commit-sha>
```

A separate scheduled workflow (`.github/workflows/cleanup-container-images.yml`) prunes old untagged image versions from the registry.

**⚠️ `SERVER_HOST` must be set explicitly when running the container.** The service defaults to binding `127.0.0.1` (see Environment Variables above), which only accepts connections from inside the container's own network namespace -- with the default, the container starts successfully but is **unreachable** through any published port. Always pass `SERVER_HOST=0.0.0.0` (the container's own network isolation is what provides the safety `127.0.0.1` would otherwise be protecting on bare metal). Since this necessarily exposes every route on every interface, consider also setting `METRICS_ALLOWED_IPS` (see Metrics & Alerting below) if `/metrics` shouldn't be reachable by everything that can reach the container:

```bash
docker run -p 3000:3000 \
  -e DATABASE_URL=postgres://user:pass@host.docker.internal/app_home \
  -e DEFAULT_USER_PASSWORD=<your-secure-password> \
  -e JWT_SECRET=<your-jwt-secret> \
  -e SERVER_HOST=0.0.0.0 \
  ghcr.io/app-home/app-home-services:latest
```

Build locally with either Docker or Podman:

```bash
docker build -t app-home-services -f Containerfile .
# or
podman build -t app-home-services -f Containerfile .
```

## Metrics & Alerting

The service exposes a Prometheus-compatible metrics endpoint:

```text
GET /metrics
```

This does not require credentials, so it should still only be reachable from inside your monitoring network/namespace where possible -- same expectation as any Prometheus scrape target. As additional, optional hardening, reachability can be restricted to a specific set of IPs via `METRICS_ALLOWED_IPS` (see below).

### Available metrics

| Metric | Type | Labels | Description |
| -------- | ------ | -------- | ------------- |
| `rate_limiter_redis_errors_total` | Counter | `scope="login"` \| `scope="refresh"` | Cumulative count of Redis errors encountered by the rate limiter. Each error means the limiter fell back to its in-memory per-instance budget (see #89) instead of enforcing the shared Redis counter, so cross-instance coordination was unavailable for that operation. Absent/zero when running on the in-memory backend (`REDIS_URL` unset), since that backend has no equivalent failure mode. Resets to 0 on process restart. Polled from the rate limiter's internal counter and republished every 15 seconds. |
| `access_token_blacklist_redis_errors_total` | Counter | — | Cumulative count of Redis errors encountered by the access-token revocation list (i.e. every time it failed open and treated an unrevoked-or-unknown token as valid instead of enforcing revocation). Absent/zero on the in-memory backend (`REDIS_URL` unset). Resets to 0 on process restart. Polled and republished every 15 seconds, same cadence as the rate limiter counter. |
| `access_token_revocation_outbox_pending` | Gauge | — | Number of journaled access-token revocations not yet flushed to Redis (rows currently in `access_token_revocation_outbox`). Only meaningful on the Redis blacklist backend (`REDIS_URL` set); the in-memory backend never journals, so this stays at 0. Republished by the durable-revocation flush worker on every sweep (see `REVOCATION_FLUSH_INTERVAL_SECONDS`). Sustained non-zero is a sign Redis has been down long enough to accumulate a backlog -- see `docs/alerting.md`. |
| `db_pool_size` | Gauge | — | Total connections (checked out + idle) currently held by the shared Postgres pool. Republished every 15 seconds from `PgPool::size()` (see #100). |
| `db_pool_idle` | Gauge | — | Idle connections currently available in the shared Postgres pool. `db_pool_size - db_pool_idle` is the number currently checked out; that number approaching `DB_MAX_CONNECTIONS` (see Database Connection Pool above) is the signal for pool exhaustion. Republished every 15 seconds from `PgPool::num_idle()` (see #100). |

### Scraping

Add a scrape target in your Prometheus config, e.g.:

```yaml
scrape_configs:
  - job_name: app-home-services
    static_configs:
      - targets: ["app-home-services:3000"]
```

### Restricting access to `/metrics`

Set `METRICS_ALLOWED_IPS` to a comma-separated list of IPs (e.g. your Prometheus server's IP) to reject requests to `/metrics` from anything else with `403 Forbidden`:

```bash
METRICS_ALLOWED_IPS=10.0.0.5,10.0.0.6
```

- Leave unset (the default) for no restriction -- `/metrics` is reachable by anything that can reach the port, same as before this option existed.
- Loopback addresses (`127.0.0.1`, `::1`) are always allowed regardless of this list, so local scraping/testing never gets locked out.
- Resolved the same trusted-proxy-aware way as rate limiting (`TRUSTED_PROXY_IPS`) -- `X-Forwarded-For`/`X-Real-IP` are honored only when the direct connection comes from a trusted proxy, so this works correctly whether Prometheus reaches the service directly or through a reverse proxy.
- This is separate from, and additional to, `SERVER_HOST` defaulting to `127.0.0.1` (see Security below) -- relevant once you've explicitly opted into `SERVER_HOST=0.0.0.0` (e.g. the container image) and want `/metrics` locked down without standing up a full reverse-proxy/auth setup.

### Alerting

Example alert rules live in `prometheus/alerts.yml`:

- **`RedisRateLimiterDegraded`** (`severity: warning`) fires when `rate_limiter_redis_errors_total` increases at all within a 5-minute window -- the limiter is running on its in-memory per-instance budget instead of the shared Redis counters (see #89).
- **`RedisAccessTokenBlacklistFailingOpen`** (`severity: critical`) fires the same way on `access_token_blacklist_redis_errors_total` -- rated a notch above the rate-limiter alert because a failing-open revocation list means a token the user explicitly revoked keeps working, not just a weakened brute-force defense.
- **`AccessTokenRevocationBacklogAccumulating`** (`severity: warning`) fires when the durable-revocation backlog (`access_token_revocation_outbox_pending`) stays above 0 for 5 minutes straight, meaning journaled revocations are accumulating because Redis has been down.

The two error-counter alerts start deliberately low (`> 0`) since there's no baseline yet for what "normal" transient Redis noise looks like in this deployment -- see [`docs/alerting.md`](docs/alerting.md) for the full reasoning and a concrete process for raising the threshold once you have a couple of weeks of real data.

`db_pool_size`/`db_pool_idle` don't have a dedicated alert rule yet (see #100) -- what counts as "too close to exhaustion" depends on `DB_MAX_CONNECTIONS`, which varies by deployment, so there's no one sensible default threshold to ship. Graphing `db_pool_size - db_pool_idle` against `DB_MAX_CONNECTIONS` is a reasonable starting dashboard panel.

## Security

- Passwords hashed with bcrypt (never stored in plaintext)
- Refresh tokens hashed with bcrypt before storage
- JWT tokens signed with HMAC-SHA256
- `JWT_SECRET` must be at least 32 bytes long and have at least 8 unique characters -- the service refuses to start otherwise, and additionally warns (without refusing to start) if character diversity is still low relative to the secret's length
- `DEFAULT_USER_PASSWORD` (the seeded admin account's password) must be at least 12 characters with at least 3 of {lowercase, uppercase, digits, symbols}, and must not be a known weak/placeholder password (e.g. `admin123`, `changeme`) -- the service refuses to start otherwise, since this account has a predictable, well-known username
- No plain-text passwords in logs (structured field logging)
- Rate limiting per IP on both login and refresh (independent counters) to prevent brute-force attacks, backed by Redis for multi-instance deployments (see Rate Limiting above)
- `X-Forwarded-For`/`X-Real-IP` only trusted from configured reverse proxies (`TRUSTED_PROXY_IPS`), preventing rate-limit bypass via header spoofing
- Password login always performs exactly one bcrypt verification (real or a fixed-cost dummy), closing the timing side-channel that would otherwise reveal whether a username exists; a uniform 50 ms delay is layered on top as additional defense-in-depth
- CORS denied by default (same-origin only)
- HTTP server binds to `127.0.0.1` by default (loopback only) -- see the `SERVER_HOST` note under Environment Variables and Container Image above for when and how to change this
- `/metrics` requires no credentials but can be restricted to an IP allowlist (`METRICS_ALLOWED_IPS`), unrestricted by default -- see Metrics & Alerting above
- Startup aborts on database connection failure, default-user seed check failure, or Redis connection failure (when configured)
- `/api/health` actively checks database connectivity (`SELECT 1` with a 2s timeout, `503` on failure) rather than always reporting healthy -- see Database Connection Pool above
- Session state transitions are one-way (active → inactive)
- Access tokens are revocable: each carries a unique `jti`, logout blacklists the presented token until its natural expiry, and every authenticated request re-checks the revocation list (see #88). The blacklist is backed by Redis when `REDIS_URL` is set (shared across instances) and is in-memory otherwise (single instance only); if the backend is unavailable the check fails open and treats the token as not revoked. Revocation is *durable* on the Redis backend (see #140): if Redis rejects a revoke at logout time, it's journaled in Postgres and a background worker retries it until it lands, so a Redis outage can delay -- but never silently drop -- a revocation (the journal lives in Postgres, so only a simultaneous Redis *and* Postgres outage could drop one)
- Sessions record the `auth_method` used to create them ("password" / "google_oauth"), so logout/refresh audit entries reflect the real method instead of assuming one
- Redis connections support password auth (`redis://:password@host:port`); TLS is not crate-native today -- see `docs/redis-security.md` for the documented decision and when to revisit it
- `admin` never queries `auth`'s `users` table directly -- identity fields are read through the `UserDirectory` port, and role data lives in admin's own `user_roles` table (see `docs/modules/admin.md`)
