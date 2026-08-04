# Contract: PUT /api/admin/users/{id}/role

## Endpoint

`PUT /api/admin/users/{id}/role`
`Authorization: Bearer <admin_access_token>`

## Path Parameters

| Name | Type | Description |
|------|------|-------------|
| `id` | UUID | User ID |

## Request

```json
{
    "role": "admin"
}
```

## Response 200 (Success)

```json
{
    "id": "0192e4a0-5b7c-7b00-8000-000000000001",
    "username": "alice",
    "email": "alice@example.com",
    "display_name": "Alice",
    "role": "admin",
    "auth_provider": "local",
    "created_at": "2026-07-19T12:00:00Z",
    "updated_at": "2026-07-19T12:00:01Z"
}
```

## Response 400 (Invalid Role)

```json
{
    "error": "role must be 'user' or 'admin'"
}
```

## Response 401 (Unauthenticated)

```json
{
    "error": "Unauthorized"
}
```

## Response 403 (Forbidden)

```json
{
    "error": "Forbidden: admin access required"
}
```

## Response 403 (Self Role Change)

Returned when an admin tries to change their own role:

```json
{
    "error": "Cannot change your own role"
}
```

## Response 404 (Not Found)

```json
{
    "error": "User not found"
}
```

## Notes

- Requires JWT authentication with admin role
- An admin cannot change their own role (returns 403) — see #92
- Accepts `"user"` or `"admin"` as valid role values
- Returns the updated user with new role
