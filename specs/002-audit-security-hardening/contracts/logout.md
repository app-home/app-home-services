# Contract: POST /api/auth/logout

## Request

`POST /api/auth/logout`
`Authorization: Bearer <access_token>`

```json
{
    "session_id": "uuid"
}
```

## Response 200 (Success)

```json
{
    "status": "logged_out"
}
```

## Response 400 (Invalid session)

```json
{
    "error": "Invalid session"
}
```

## Response 401 (Unauthenticated)

```json
{
    "error": "Authentication required"
}
```

## Response 500 (Internal server error)

```json
{
    "error": "Internal server error"
}
```

## Notes

- Invalidates the session associated with the session_id
- Records a "logout" event in the user_actions audit trail
- The access token itself remains valid until its natural expiry (15 min), but the refresh token is immediately revoked
- If the session is already invalidated or expired, returns 200 (idempotent)
