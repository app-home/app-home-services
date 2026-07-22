# Module: Admin (`crates/admin/`)

## Purpose

Admin user management bounded context. Provides admin-only endpoints for listing users, viewing user details, and updating user roles. Extends the `users` table with a `role` column (migration 007). Admin access is gated by JWT authentication + DB role check.

## Dependencies

| Crate | Role |
|-------|------|
| `shared` | `AuthenticatedUser` (JWT extractor), `ErrorResponse` |

No dependency on `auth` — only on `shared`.

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
| `is_admin(user_id)` | Check admin permission (DB role query) |
| `update_role(user_id, role)` | Update user role, return updated user |

### Use Cases

| Use Case | Input | Description |
|----------|-------|-------------|
| `list_users` | `repo` | Returns `Vec<AdminUser>` |
| `get_user` | `repo`, `user_id` | Returns `AdminUser` or `NotFound` |
| `update_user_role` | `repo`, `user_id`, `role: &str` | Parses role into `Role` VO, calls `repo.update_role` |

## Adapter Layer

### Inbound (HTTP Handlers)

All handlers require **JWT Bearer** + **admin role** (`is_admin()` DB check → 403 `AdminGuard` if false):

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
| `PostgresAdminRepo` | `AdminRepository` | SQLx queries against `users` table with `role` column |

## Database

Extends `users` table via migration 007:

```sql
ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'user'
  CHECK (role IN ('user', 'admin'));
```

**This is the strongest cross-context coupling in the codebase.** `admin` doesn't
own a table of its own -- every method on `PostgresAdminRepo` (`list_users`,
`get_user`, `is_admin`, `update_role`) runs SQL directly against `users`, a table
created and conceptually owned by `auth` (migration 001). `admin` effectively owns
one column (`role`) on someone else's table. See
[`docs/adr/0001-modular-monolith.md`](../adr/0001-modular-monolith.md) for why this
is acceptable at the current stage and exactly what would need to change (an `auth`-
owned API for `admin` to call, instead of direct SQL) before `admin` could be
extracted into its own service.

Data model: [`specs/006-admin/data-model.md`](../../specs/006-admin/data-model.md)

## Integration

In `main.rs`:

1. `PostgresAdminRepo` created from pool → wrapped in `Arc`
2. Injected as `Extension(admin_repo)` at router level
3. Routes: `GET /api/admin/users`, `GET /api/admin/users/{id}`, `PUT /api/admin/users/{id}/role`
4. `AdminGuard` (implements `IntoResponse`) returns 403 when `is_admin()` fails — no dependency on `auth::AppState`
