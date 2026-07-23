# Module: Admin (`crates/admin/`)

## Purpose

Admin user management bounded context. Provides admin-only endpoints for listing users, viewing user details, and updating user roles. Owns its own `user_roles` table (migration 008). Admin access is gated by JWT authentication + role check.

## Dependencies

| Crate | Role |
|-------|------|
| `shared` | `AuthenticatedUser` (JWT extractor), `ErrorResponse`, `UserDirectory` (port for reading user identity, implemented by `auth`) |

No dependency on `auth` — only on `shared`. `admin` reads user identity through `shared::user_directory::UserDirectory`, injected as `Arc<dyn UserDirectory>` at the composition root (`main.rs`), which wires in `auth`'s concrete implementation without `admin` depending on the `auth` crate itself.

## Domain Layer

### Entity

`AdminUser` — `id`, `username: Option<String>`, `email`, `display_name`, `auth_provider`, `role`, `created_at`, `updated_at`

### Value Object

`Role` — Two variants: `User`, `Admin`. Provides `as_str()`. Parsed from input strings at the adapter layer.

### Domain Errors

`AdminError` enum: `NotFound(Uuid)`, `Unauthorized`, `InvalidValue(String)`, `InternalError(String)`

## Application Layer (Use Cases)

### Ports

**`AdminRepository`** trait:

| Method | Description |
|--------|-------------|
| `list_users()` | Return all users |
| `get_user(user_id)` | Return single user by ID |
| `is_admin(user_id)` | Check admin permission (role lookup) |
| `update_role(user_id, role)` | Update user role, return updated user |

### Use Cases

| Use Case | Input | Description |
|----------|-------|-------------|
| `list_users` | `repo` | Returns `Vec<AdminUser>` |
| `get_user` | `repo`, `user_id` | Returns `AdminUser` or `NotFound` |
| `update_user_role` | `repo`, `user_id`, `role: &str` | Parses role into `Role` VO, calls `repo.update_role` |

## Adapter Layer

### Inbound (HTTP Handlers)

All handlers require **JWT Bearer** + **admin role** (`is_admin()` check → 403 `AdminGuard` if false):

| Method | Path | Body | Response 200 | Errors |
|--------|------|------|--------------|--------|
| `GET` | `/api/admin/users` | — | `Vec<UserResponse>` | 401, 403, 500 |
| `GET` | `/api/admin/users/{id}` | — | `UserResponse` | 401, 403, 404, 500 |
| `PUT` | `/api/admin/users/{id}/role` | `UpdateRoleRequest {role}` | `UserResponse` | 400, 401, 403, 404, 500 |

**Response DTOs**:
- `UserResponse` — `id`, `username?`, `email`, `display_name`, `role`, `auth_provider`, `created_at`, `updated_at`
- `UpdateRoleRequest` — `role: String`

**Contracts**: [list-users](../../specs/006-admin/contracts/list-users.md) · [get-user](../../specs/006-admin/contracts/get-user.md) · [update-user-role](../../specs/006-admin/contracts/update-user-role.md)

### Outbound

| Adapter | Implements | Description |
|---------|-----------|-------------|
| `PostgresAdminRepo` | `AdminRepository` | Queries its own `user_roles` table for role data; delegates identity fields to an injected `Arc<dyn UserDirectory>` |

## Database

**Table**: `user_roles` (owned by `admin`, migration 008)

```sql
CREATE TABLE user_roles (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL DEFAULT 'user' CHECK (role IN ('user', 'admin')),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

**Resolved coupling.** This used to be the strongest cross-context coupling in the
codebase: every `PostgresAdminRepo` method ran SQL directly against `users`, a table
owned by `auth` (migration 001), with `admin` effectively owning one borrowed column
(`role`, migration 007) on someone else's table. As of migration 008, `admin` owns
`user_roles` outright and never queries `users` -- identity fields come through
`UserDirectory` instead (see Dependencies above). The FK into `users(id)` remains for
referential integrity, since both contexts still share one database today; see
[`docs/adr/0001-modular-monolith.md`](../adr/0001-modular-monolith.md) for what
extracting `admin` into its own service would still require beyond this (splitting
the schema, replacing `UserDirectory`'s in-process call with a network one).

Data model: [`specs/006-admin/data-model.md`](../../specs/006-admin/data-model.md)

## Integration

In `main.rs`:

1. `PostgresUserDirectory` (from `auth`) created from pool → wrapped in `Arc<dyn UserDirectory>`
2. `PostgresAdminRepo` created from pool + that `UserDirectory` handle → wrapped in `Arc`
3. Injected as `Extension(admin_repo)` at router level
4. Routes: `GET /api/admin/users`, `GET /api/admin/users/{id}`, `PUT /api/admin/users/{id}/role`
5. `AdminGuard` (implements `IntoResponse`) returns 403 when `is_admin()` fails — no dependency on `auth::AppState`
