# Contract: POST /api/auth/refresh

## Request

`POST /api/auth/refresh`
`Content-Type: application/json`

```json
{
    "refresh_token": "<jwt-7d>"
}
```

## Response 200 (Success)

```json
{
    "access_token": "<new-jwt-15min>",
    "refresh_token": "<new-jwt-7d>"
}
```

## Response 401 (Invalid or expired refresh token)

```json
{
    "error": "Invalid or expired refresh token"
}
```

## Response 401 (Session inactive)

```json
{
    "error": "Session has been invalidated"
}
```

## Notes

- On successful refresh, the old refresh token is rotated (invalidated) and a new session is created
- If the refresh token is valid but the corresponding session has `is_active = false`, the refresh is rejected (the session was logged out)
