# Contract: POST /api/auth/login/password

## Request

`POST /api/auth/login/password`
`Content-Type: application/json`

```json
{
    "username": "admin",
    "password": "secret123"
}
```

## Response 200 (Success)

```json
{
    "access_token": "<jwt-15min>",
    "refresh_token": "<jwt-7d>",
    "user_id": "uuid"
}
```

## Response 401 (Invalid credentials)

```json
{
    "error": "Invalid username or password"
}
```

## Response 422 (Validation error)

```json
{
    "error": "Username and password are required"
}
```

## Response 429 (Rate limited)

```json
{
    "error": "Too many login attempts. Please try again later."
}
```
