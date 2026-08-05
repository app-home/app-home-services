# Contract: GET /api/admin/users

## Endpoint

`GET /api/admin/users`
`Authorization: Bearer <admin_access_token>`

### Query Parameters

| Parameter   | Type | Required | Default | Description                                                     |
|-------------|------|----------|---------|-----------------------------------------------------------------|
| `page`      | u32  | No       | 1       | 1-based page number. Must be >= 1. 0 returns 400.               |
| `per_page`  | u32  | No       | 100     | Page size, clamped to 1..500. Beyond 500 silently capped at 500.|

## Response 200 (Success)

Users are ordered by `created_at` descending, then `id` descending, in a
pagination envelope. The `id` tie-breaker makes page membership deterministic
when users share a creation timestamp.

```json
{
    "items": [
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
    ],
    "page": 1,
    "per_page": 100,
    "total": 1
}
```

## Response 400 (Invalid page)

```json
{
    "error": "page must be >= 1"
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

## Response 500

```json
{
    "error": "Internal server error"
}
```

## Notes

- Requires JWT authentication with admin role.
- Returns a page of users ordered by `created_at` descending, then `id` descending.
- `username` may be null for Google-authenticated users.
- Server-enforced bounds prevent single-request memory exhaustion (see #101).
