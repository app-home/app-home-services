# Contract: Login with Username/Password

## Endpoint

`POST /api/auth/login/password`

## Request

```json
{
  "username": "admin",
  "password": "secret123"
}
```

## Success Response (200)

```json
{
  "status": "authenticated",
  "user_id": "uuid-v7-string"
}
```

## Error Responses

### 401 - Invalid credentials

```json
{
  "error": "Invalid username or password"
}
```
*Same message for non-existent username and incorrect password.*

### 422 - Validation error

```json
{
  "error": "Username and password are required"
}
```
*When fields are missing or empty.*

## Notes

- Session token generation is out of scope.
- The response must never include `password_hash` or any sensitive fields.
- Generic error message does not distinguish "user not found" from "wrong password".
