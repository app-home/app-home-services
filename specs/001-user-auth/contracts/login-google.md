# Contract: Login with Google OAuth

## Endpoint

`POST /api/auth/login/google`

## Request

```json
{
  "id_token": "google-oauth-id-token-string"
}
```

## Success Response (200) - Existing User

```json
{
  "status": "authenticated",
  "user_id": "uuid-v7-string",
  "is_new_user": false
}
```

## Success Response (200) - Newly Created User

```json
{
  "status": "authenticated",
  "user_id": "uuid-v7-string",
  "is_new_user": true
}
```

## Error Responses

### 401 - Invalid token

```json
{
  "error": "Authentication failed"
}
```
*Token is invalid, expired, or has wrong issuer/audience.*

### 422 - Validation error

```json
{
  "error": "ID token is required"
}
```
*When `id_token` field is missing.*

## Notes

- The system verifies the Google ID token using Google's public keys (JWKS).
- Verification includes: signature, issuer (`accounts.google.com`), audience (client ID).
- If no user exists matching the Google email, a new user record is created.
- Session token generation is out of scope.
