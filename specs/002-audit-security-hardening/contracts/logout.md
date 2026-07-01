# Contract: POST /api/auth/logout

## Request

`POST /api/auth/logout`
`Authorization: Bearer <access_token>`

No request body required.

## Response 200 (Success)

```json
{
    "status": "logged_out"
}
```

## Response 401 (Unauthenticated)

```json
{
    "error": "Authentication required"
}
```

## Notes

- Invalidates the session associated with the access token
- Records a "logout" event in the user_actions audit trail
- The access token itself remains valid until its natural expiry (15 min), but the refresh token is immediately revoked
- If the session is already invalidated or expired, returns 200 (idempotent)
