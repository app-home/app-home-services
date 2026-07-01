# Quickstart: User Authentication

## Prerequisites

- Rust toolchain (Edition 2024)
- PostgreSQL 16+ running locally
- A Google OAuth 2.0 client ID (for Google login testing)
- Environment variables configured (see `infrastructure/config/settings.rs`)

## Setup

1. Clone the repository and navigate to the project root.
2. Copy `.env.example` to `.env` and fill in:
   ```env
   DATABASE_URL=postgres://user:pass@localhost/app_home
   DEFAULT_USER_USERNAME=admin
   DEFAULT_USER_PASSWORD=<your-secure-password>
   DEFAULT_USER_EMAIL=admin@example.com
   GOOGLE_CLIENT_ID=<your-google-client-id>
   ```
3. Run database setup:
   ```bash
   cargo run -- setup
   ```
   This runs migrations and seeds the default user.

4. Start the server:
   ```bash
   cargo run
   ```

## Validation Scenarios

### Scenario 1: Default user login

```bash
curl -X POST http://localhost:3000/api/auth/login/password \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "<your-secure-password>"}'
```

**Expected**:
```json
{"status": "authenticated", "user_id": "<uuid>"}
```

### Scenario 2: Wrong password

```bash
curl -X POST http://localhost:3000/api/auth/login/password \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "wrongpassword"}'
```

**Expected**:
```json
{"error": "Invalid username or password"}
```

### Scenario 3: Non-existent username

```bash
curl -X POST http://localhost:3000/api/auth/login/password \
  -H "Content-Type: application/json" \
  -d '{"username": "nonexistent", "password": "anything"}'
```

**Expected**: Same error message as Scenario 2:
```json
{"error": "Invalid username or password"}
```

### Scenario 4: Google login (requires valid Google ID token)

Obtain a Google ID token from your test Google account, then:

```bash
curl -X POST http://localhost:3000/api/auth/login/google \
  -H "Content-Type: application/json" \
  -d '{"id_token": "<your-google-id-token>"}'
```

**Expected** (first time):
```json
{"status": "authenticated", "user_id": "<uuid>", "is_new_user": true}
```

**Expected** (returning user):
```json
{"status": "authenticated", "user_id": "<uuid>", "is_new_user": false}
```

### Scenario 5: Invalid Google ID token

```bash
curl -X POST http://localhost:3000/api/auth/login/google \
  -H "Content-Type: application/json" \
  -d '{"id_token": "invalid-token"}'
```

**Expected**:
```json
{"error": "Authentication failed"}
```

## Running Tests

```bash
# Run all tests
cargo test

# Run unit tests only
cargo test --lib

# Run integration tests (requires running PostgreSQL)
cargo test --test integration
```

## Checking Audit Log

Query the database directly to verify audit entries:

```sql
SELECT u.username, u.email, ua.auth_method, ua.created_at
FROM user_actions ua
JOIN users u ON u.id = ua.user_id
ORDER BY ua.created_at DESC;
```

Each successful login should produce one row in `user_actions`.

## Related Documents

- [Specification](spec.md)
- [Data Model](data-model.md)
- [Contracts](contracts/login-password.md)
- [Contracts](contracts/login-google.md)
