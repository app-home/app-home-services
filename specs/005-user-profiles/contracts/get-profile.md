# Contract: GET /api/profile

## Endpoint

`GET /api/profile`
`Authorization: Bearer <access_token>`

## Response 200 (Success)

```json
{
    "user_id": "0192e4a0-5b7c-7b00-8000-000000000001",
    "avatar_url": "https://example.com/avatar.png",
    "bio": "Hello, world!",
    "updated_at": "2026-07-19T12:00:00Z"
}
```

## Response 401 (Unauthenticated)

```json
{
    "error": "Unauthorized"
}
```

## Response 404 (Not Found)

```json
{
    "error": "Profile not found"
}
```

## Notes

- Returns the authenticated user's profile
- Auto-creates a profile on first access if one doesn't exist
- `avatar_url` and `bio` may be null if not set
