# Module: Profiles (`crates/profiles/`)

## Purpose

User profiles bounded context. Manages per-user profile data (avatar URL, bio) stored in the `user_profiles` table. Extracted as a separate bounded context from auth, with its own domain logic, repository port, and HTTP handlers. JWT authentication via the shared `AuthenticatedUser` extractor.

## Dependencies

| Crate | Role |
|-------|------|
| `shared` | `AuthenticatedUser` (JWT extractor), `ErrorResponse` |

No dependency on `auth` — only on `shared`.

## Domain Layer

### Entity

`UserProfile` — `user_id: Uuid`, `avatar_url: Option<AvatarUrl>`, `bio: Option<Bio>`, `updated_at: DateTime<Utc>`

### Value Objects

| Value Object | Description | Validation |
|-------------|-------------|------------|
| `AvatarUrl` | User avatar URL string | Max 500 chars |
| `Bio` | User bio text | Max 2000 chars |

### Domain Errors

`ProfileError` enum: `NotFound(Uuid)`, `InvalidValue(String)`, `InternalError(String)`

## Application Layer (Use Cases)

### Ports

**`ProfileRepository`** trait:
- `find_by_user_id(user_id: Uuid)` → `Option<UserProfile>`
- `upsert(profile: &UserProfile)` → `()`

### Use Cases

| Use Case | Input | Description |
|----------|-------|-------------|
| `get_profile` | `repo`, `user_id` | Fetch profile; returns `NotFound` if none |
| `update_profile` | `repo`, `user_id`, `avatar_url?`, `bio?` | Validates via value objects, upserts profile |

## Adapter Layer

### Inbound (HTTP Handlers)

| Method | Path | Auth | Body | Response 200 | Errors |
|--------|------|------|------|--------------|--------|
| `GET` | `/api/profile` | Bearer JWT | — | `ProfileResponse` | 401, 404, 500 |
| `PUT` | `/api/profile` | Bearer JWT | `UpdateProfileRequest` | `ProfileResponse` | 400, 401, 500 |

**Response DTOs**:
- `ProfileResponse { user_id, avatar_url?, bio?, updated_at }`
- `UpdateProfileRequest { avatar_url: Option<String>, bio: Option<String> }`

Both handlers extract `AuthenticatedUser` (JWT) and resolve `ProfileRepository` from `Extension<Arc<dyn ProfileRepository>>`.

**Contracts**: [get-profile](../../specs/005-user-profiles/contracts/get-profile.md) · [update-profile](../../specs/005-user-profiles/contracts/update-profile.md)

### Outbound

| Adapter | Implements | Description |
|---------|-----------|-------------|
| `PostgresProfileRepo` | `ProfileRepository` | SQLx implementation against `user_profiles` table. `upsert` uses `INSERT ... ON CONFLICT (user_id) DO UPDATE`. |

## Database

**Table**: `user_profiles`

| Column | Type | Key |
|--------|------|-----|
| `user_id` | UUID | PK, FK → users.id |
| `avatar_url` | TEXT | nullable |
| `bio` | TEXT | nullable |
| `updated_at` | TIMESTAMPTZ | |

Data model: [`specs/005-user-profiles/data-model.md`](../../specs/005-user-profiles/data-model.md)

## Integration

In `main.rs`:

1. `PostgresProfileRepo` created from pool → wrapped in `Arc`
2. Injected as `Extension(profile_repo)` at router level
3. Routes: `GET /api/profile`, `PUT /api/profile`
4. No separate `AppState` — extracts `Arc<dyn ProfileRepository>` directly