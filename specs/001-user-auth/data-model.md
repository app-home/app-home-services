# Data Model: User Authentication

## Entities

### User

| Field | Type | Constraints |
|---|---|---|
| `id` | UUID (v7) | Primary key, generated server-side |
| `username` | VARCHAR(255) | Nullable; unique when non-null; set for default user only |
| `email` | VARCHAR(255) | Unique; from Google profile for OAuth users |
| `display_name` | VARCHAR(255) | From Google profile or set for default user |
| `password_hash` | VARCHAR(255) | Nullable; null for Google-only users; bcrypt hash for default user |
| `auth_provider` | VARCHAR(50) | `"local"` or `"google"` |
| `created_at` | TIMESTAMPTZ | Auto-set on creation |
| `updated_at` | TIMESTAMPTZ | Auto-updated on modification |

**Validation Rules**:
- If `auth_provider = "local"`, then `username` and `password_hash` must be non-null.
- If `auth_provider = "google"`, then `email` must be non-null and `password_hash` must be null.
- `email` must be a valid email format.
- `password_hash` must never be returned in any API response.

**State Transitions**:
- Created on first Google login (auto-registration).
- Created during database seeding (default user).
- No update or delete transitions in scope.

### UserAction (Audit Log)

| Field | Type | Constraints |
|---|---|---|
| `id` | UUID (v7) | Primary key, generated server-side |
| `user_id` | UUID | Foreign key to `users.id`, not null |
| `auth_method` | VARCHAR(50) | `"password"` or `"google_oauth"` |
| `created_at` | TIMESTAMPTZ | Auto-set on creation |

**Validation Rules**:
- `auth_method` must be one of `"password"` or `"google_oauth"`.
- `user_id` must reference an existing user.

**State Transitions**:
- Append-only. Records are never modified or deleted.

## Relationships

```
User 1───* UserAction
```

- One user can have many user_action records (one per successful login).
- Each user_action belongs to exactly one user.

## Database Migrations

### Migration 1: Create users table

```sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(255) UNIQUE,
    email VARCHAR(255) UNIQUE NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    password_hash VARCHAR(255),
    auth_provider VARCHAR(50) NOT NULL DEFAULT 'local',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_auth_provider ON users(auth_provider);
```

### Migration 2: Create user_actions table

```sql
CREATE TABLE user_actions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    auth_method VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_user_actions_user_id ON user_actions(user_id);
CREATE INDEX idx_user_actions_created_at ON user_actions(created_at);
```

### Migration 3: Seed default user

```sql
-- Default user credentials set via environment/config, not hard-coded here
-- This migration is placeholder; actual seeding uses bcrypt hash from config
INSERT INTO users (id, username, email, display_name, password_hash, auth_provider)
VALUES (
    gen_random_uuid(),
    'admin',
    'admin@example.com',
    'Administrator',
    '<bcrypt_hash_from_config>',
    'local'
);
```

**Note**: Migration 3 is a template. The actual seed migration should use a configurable hash generated at deployment time or via a setup script.
