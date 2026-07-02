# app-home-services

User authentication service supporting local password login, Google OAuth, session-based JWT authentication, audit trail, rate limiting, and CORS restrictions.

## Requirements

- Rust 2024 edition (nightly)
- PostgreSQL 14+

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

   Migrations are applied automatically on startup (via `sqlx::migrate!`). On first run, the default local user is also seeded. The process aborts with a clear error if the database is unreachable.

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
| `RATE_LIMIT_WINDOW_SECONDS` | No | `300` | Rate limit sliding window in seconds (default: 5 min) |
| `CORS_ALLOWED_ORIGINS` | No | — | Comma-separated allowed origins; empty = same-origin only |

## API Endpoints

### Authentication

| Method | Path | Auth | Description |
| -------- | ------ | ------ | ------------- |
| POST | `/api/auth/login/password` | No | Login with username/password |
| POST | `/api/auth/login/google` | No | Login with Google OAuth ID token |
| POST | `/api/auth/logout` | Bearer | Invalidate a session |
| POST | `/api/auth/refresh` | No | Rotate refresh token for a new access + refresh pair |

### System

| Method | Path | Auth | Description |
| -------- | ------ | ------ | ------------- |
| GET | `/api/health` | No | Health check |

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

Failed logins return `401` with `{"error": "Invalid username or password"}`. A uniform 50 ms delay is applied to all failure responses to prevent timing side-channel attacks.

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

Failed login attempts are tracked per IP address using an in-memory sliding window (default: 10 attempts per 5 minutes). When the limit is exceeded, the endpoint returns `429 Too Many Requests`. A successful login resets the counter for that IP.

### CORS

Cross-origin requests are restricted to origins listed in `CORS_ALLOWED_ORIGINS` (comma-separated). When the variable is empty, all cross-origin requests are denied (same-origin policy only).

## Architecture

The project follows **Hexagonal Architecture (Ports & Adapters)**:

```text
                      ┌──────────┐
                      │  Axum    │  (inbound adapters — HTTP handlers)
                      │ Routes   │
                      └────┬─────┘
                           │
                 ┌─────────▼─────────┐
                 │   Use Cases       │  (application layer — orchestration)
                 └────┬─────────┬────┘
                      │         │
             ┌────────▼──┐  ┌──▼────────┐
             │ Ports     │  │ Ports     │  (application/ports — traits)
             │ (traits)  │  │ (traits)  │
             └────┬──────┘  └─────┬─────┘
                  │               │
        ┌─────────▼──────┐  ┌────▼──────────┐
        │ PostgresRepos  │  │ JwtService    │  (outbound adapters)
        │ RateLimiter    │  │ GoogleAuth    │
        └────────────────┘  └───────────────┘
```

### Key Modules

| Layer | Path | Description |
| ------- | ------ | ------------- |
| Domain | `src/domain/entities/` | `User`, `Session`, `UserAction` entities |
| Domain | `src/domain/errors.rs` | `AuthError` enum with typed error variants |
| Application | `src/application/ports/` | Traits: `UserRepository`, `SessionRepository`, `JwtService`, `RateLimiter`, `AuthProvider` |
| Application | `src/application/use_cases/` | `login_with_password`, `login_with_google`, `logout`, `refresh_token`, `record_audit_entry` |
| Adapters | `src/adapters/inbound/` | HTTP handlers + auth middleware |
| Adapters | `src/adapters/outbound/` | `PostgresUserRepo`, `PostgresSessionRepo`, `JwtServiceImpl`, `MemoryRateLimiter`, `GoogleAuthProvider` |
| Infrastructure | `src/infrastructure/` | Config, database pool, telemetry |

## Migrations

| File | Description |
| ------ | ------------- |
| `001_create_users_table.sql` | Users table with local + Google auth support |
| `002_create_user_actions_table.sql` | Audit trail for auth events |
| `003_create_sessions_table.sql` | Sessions table for JWT refresh token management |
| `004_extend_user_actions.sql` | Adds `session_id` and `event_type` to user_actions |

Migrations run automatically on startup.

## Testing

```bash
# Run all unit tests (no database required)
cargo test

# Run integration tests (requires running PostgreSQL + server)
cargo test -- --ignored
```

- **35 unit tests**: Session entity, JWT service, rate limiter, user action audit, password hashing
- **21 integration tests** (ignored by default): Login, logout, refresh, CORS, rate limiting, startup hardening

## Security

- Passwords hashed with bcrypt (never stored in plaintext)
- Refresh tokens hashed with bcrypt before storage
- JWT tokens signed with HMAC-SHA256
- No plain-text passwords in logs (structured field logging)
- Rate limiting per IP to prevent brute-force attacks
- Uniform 50 ms delay on all login failures to prevent timing attacks
- CORS denied by default (same-origin only)
- Startup aborts on database connection failure
- Session state transitions are one-way (active → inactive)
