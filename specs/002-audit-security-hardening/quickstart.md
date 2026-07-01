# Quickstart: Audit & Security Hardening

## Prerequisites

- PostgreSQL running with `app_home` database created
- `.env` configured with `DATABASE_URL`, `DEFAULT_USER_PASSWORD`, `JWT_SECRET`

## Setup

```bash
cp .env.example .env
# Edit .env with your values
cargo run
```

## Validation Scenarios

### Scenario 1: Startup hardening

```bash
# Fresh database — expect "Default user created successfully" in logs
cargo run 2>&1 | grep "default.user"

# Second run — expect "Default user already exists" in logs
cargo run 2>&1 | grep "default.user"

# Unreachable database — expect exit with error
DATABASE_URL=postgres://invalid@localhost/nonexist cargo run
# Expected: process exits with error message
```

### Scenario 2: Login with tokens

```bash
# Login and capture tokens
RESPONSE=$(curl -s -X POST localhost:3000/api/auth/login/password \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"your-password"}')
ACCESS_TOKEN=$(echo $RESPONSE | jq -r '.access_token')
REFRESH_TOKEN=$(echo $RESPONSE | jq -r '.refresh_token')

# Use access token (verify authenticated access)
# (Protected endpoints will validate this token)
```

### Scenario 3: Token refresh

```bash
# Refresh the access token
REFRESH_RESPONSE=$(curl -s -X POST localhost:3000/api/auth/refresh \
  -H 'Content-Type: application/json' \
  -d "{\"refresh_token\":\"$REFRESH_TOKEN\"}")
NEW_ACCESS=$(echo $REFRESH_RESPONSE | jq -r '.access_token')
NEW_REFRESH=$(echo $REFRESH_RESPONSE | jq -r '.refresh_token')
# Old refresh_token is now invalid
```

### Scenario 4: Logout

```bash
# Logout
curl -s -X POST localhost:3000/api/auth/logout \
  -H "Authorization: Bearer $ACCESS_TOKEN"
# Expected: {"status":"logged_out"}

# Verify old refresh token no longer works
curl -s -X POST localhost:3000/api/auth/refresh \
  -H 'Content-Type: application/json' \
  -d "{\"refresh_token\":\"$OLD_REFRESH_TOKEN\"}"
# Expected: 401 error
```

### Scenario 5: Rate limiting

```bash
# Send 10+ failed login attempts rapidly
for i in $(seq 1 12); do
  curl -s -X POST localhost:3000/api/auth/login/password \
    -H 'Content-Type: application/json' \
    -d '{"username":"admin","password":"wrong"}'
done
# After ~10 attempts, expect 429 Too Many Requests
```

### Scenario 6: CORS restriction

```bash
# Request from disallowed origin
curl -s -X POST localhost:3000/api/auth/login/password \
  -H 'Origin: https://evil.com' \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"wrong"}'
# Expected: No Access-Control-Allow-Origin header in response
```

## Expected Outcomes

| Scenario | Expected |
|----------|----------|
| Fresh DB startup | "Default user created successfully" in logs |
| Subsequent startup | "Default user already exists" in logs |
| Unreachable DB | Process exits with error |
| Valid login | HTTP 200 with access_token + refresh_token |
| Invalid login | HTTP 401 with error message |
| Rate limit exceeded | HTTP 429 with error message |
| Valid logout | HTTP 200, session invalidated |
| Refresh after logout | HTTP 401 |
| CORS disallowed origin | No Access-Control-Allow-Origin header |
