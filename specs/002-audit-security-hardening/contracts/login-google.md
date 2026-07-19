# Contract: POST /api/auth/login/google

## Request

`POST /api/auth/login/google`
`Content-Type: application/json`

```json
{
    "id_token": "<google-id-token>"
}
```

## Response 200 (Success)

```json
{
    "access_token": "<jwt-15min>",
    "refresh_token": "<jwt-7d>",
    "user_id": "uuid",
    "is_new_user": false
}
```

## Response 401 (Token verification failed)

```json
{
    "error": "Authentication failed"
}
```

## Response 422 (Validation error)

```json
{
    "error": "ID token is required"
}
```

## Response 429 (Rate limit exceeded)

```json
{
    "error": "Rate limit exceeded"
}
```

## Response 500 (Internal server error)

```json
{
    "error": "Internal server error"
}
```
