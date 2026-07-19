# Contract: PUT /api/profile

## Endpoint

`PUT /api/profile`
`Authorization: Bearer <access_token>`

## Request

```json
{
    "avatar_url": "https://example.com/new-avatar.png",
    "bio": "Updated bio text"
}
```

## Response 200 (Success)

```json
{
    "user_id": "0192e4a0-5b7c-7b00-8000-000000000001",
    "avatar_url": "https://example.com/new-avatar.png",
    "bio": "Updated bio text",
    "updated_at": "2026-07-19T12:00:01Z"
}
```

## Response 400 (Validation Error)

```json
{
    "error": "avatar_url exceeds maximum length of 500 characters"
}
```

## Response 401 (Unauthenticated)

```json
{
    "error": "Unauthorized"
}
```

## Notes

- Updates the authenticated user's profile
- Only provided fields are updated; omitted or null fields remain unchanged
- `avatar_url` max length: 500 characters
- `bio` max length: 2000 characters
