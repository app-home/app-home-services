# Data Model: Audit & Security Hardening

## Entities

### Session (New)

Represents an authenticated user session. Created on login, invalidated on logout.

| Field | Type | Description |
|-------|------|-------------|
| `id` | UUID (PK) | Generated via `Uuid::now_v7()` |
| `user_id` | UUID (FK → users) | The authenticated user |
| `refresh_token_hash` | String | Bcrypt hash of the refresh JWT |
| `expires_at` | DateTime<Utc> | When the session expires (7 days from creation) |
| `is_active` | Boolean | Whether the session is still valid (set to false on logout) |
| `created_at` | DateTime<Utc> | When the session was created |

**Validation rules**:
- `refresh_token_hash` must not be empty
- `expires_at` must be in the future when created
- `is_active` transitions: `true` → `false` (one-way, irreversible)
- A user may have multiple active sessions (concurrent logins)

### UserAction (Extended)

Extended to track login and logout events with session linkage.

| Field | Type | Description |
|-------|------|-------------|
| `id` | UUID (PK) | Generated via `Uuid::now_v7()` |
| `user_id` | UUID (FK → users) | The user who performed the action |
| `session_id` | UUID (FK → sessions, nullable) | NEW: Links login↔logout pairs |
| `event_type` | String | NEW: `"login"`, `"logout"`, or `"refresh"` |
| `auth_method` | String | `"password"` or `"google_oauth"` |
| `created_at` | DateTime<Utc> | When the action occurred |

**Validation rules**:
- `event_type` must be one of: `login`, `logout`, `refresh`
- `session_id` is required for `logout` and `refresh` events; optional for `login`

### User (Existing, Unchanged)

| Field | Type |
|-------|------|
| `id` | UUID (PK) |
| `username` | String? (unique) |
| `email` | String (unique) |
| `display_name` | String |
| `password_hash` | String? |
| `auth_provider` | String (`"local"` or `"google"`) |
| `created_at` | DateTime<Utc> |
| `updated_at` | DateTime<Utc> |

## Database Schema Changes

### Migration 003: `sessions` table (New)

```sql
CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    refresh_token_hash VARCHAR(255) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);
CREATE INDEX idx_sessions_active ON sessions(is_active) WHERE is_active = TRUE;
```

### Migration 004: Extend `user_actions` table

```sql
ALTER TABLE user_actions
    ADD COLUMN session_id UUID REFERENCES sessions(id) ON DELETE SET NULL,
    ADD COLUMN event_type VARCHAR(20) NOT NULL DEFAULT 'login';

CREATE INDEX idx_user_actions_session_id ON user_actions(session_id);
CREATE INDEX idx_user_actions_event_type ON user_actions(event_type);
```

## State Transitions

### Session Lifecycle

```
[LOGIN] ──> session(is_active=true) ──> [LOGOUT] ──> session(is_active=false) ──> [EXPIRED]
                                              └── or ──> session expires at `expires_at`
                                                     ──> [REFRESH] ──> old session invalidated
                                                                   └──> new session created
```
