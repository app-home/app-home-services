# Contract: GET /api/admin/users

## Endpoint

`GET /api/admin/users`
`Authorization: Bearer <admin_access_token>`

## Response 200 (Success)

```json
[
    {
        "id": "0192e4a0-5b7c-7b00-8000-000000000001",
        "username": "admin",
        "email": "admin@example.com",
        "display_name": "Administrator",
        "role": "admin",
        "auth_provider": "local",
        "created_at": "2026-07-19T12:00:00Z",
        "updated_at": "2026-07-19T12:00:00Z"
    }
]
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

## Notes

- Requires JWT authentication with admin role
- Returns all users ordered by created_at descending
- `username` may be null for Google-authenticated users
