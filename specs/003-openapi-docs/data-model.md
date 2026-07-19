# Phase 1 Data Model: OpenAPI & Swagger Documentation

**Feature**: `specs/003-openapi-docs` | **Date**: 2026-07-15

This feature introduces no database entities. The "data model" here is the set of
**HTTP wire DTOs** and the **OpenAPI document metadata** that the specification is
generated from. All types live in the inbound adapter layer (`src/adapters/inbound/`)
and derive `serde::Serialize`/`Deserialize` and `utoipa::ToSchema`. The domain and
application layers are unchanged.

---

## Request DTOs (existing structs — add `ToSchema`)

| Type | Location | Fields | Validation (as implemented) |
|------|----------|--------|------------------------------|
| `PasswordLoginRequest` | `login_routes.rs:16` | `username: String`, `password: String` | Both required & non-empty → else **422** ("Username and password are required") |
| `GoogleLoginRequest` | `oauth_callback.rs:9` | `id_token: String` | Non-empty → else **422** ("ID token is required") |
| `LogoutRequest` | `logout_routes.rs:11` | `session_id: Uuid` | Must parse as UUID (deserialization); requires valid Bearer token |
| `RefreshTokenRequest` | `refresh_routes.rs:17` | `refresh_token: String` | Non-empty → else **422** ("refresh_token is required") |

**Change required**: add `#[derive(ToSchema)]` (alongside existing `Deserialize`) and, where
`utoipa` requires visibility, ensure fields are exposable to the schema derive. Add non-sensitive
`#[schema(example = ...)]` values (e.g. `username = "jdoe"`, never a real password/token).

---

## Response DTOs (new — `src/adapters/inbound/responses.rs`)

All derive `#[derive(Serialize, ToSchema)]`. Field names & values are **identical** to the
current `serde_json::json!` output to preserve the wire contract.

### `AuthTokensResponse`
Password-login success (200).
| Field | Type | Example | Notes |
|-------|------|---------|-------|
| `status` | String | `"authenticated"` | constant |
| `user_id` | String | `"018f...-uuid"` | UUID as string |
| `access_token` | String | `"<jwt>"` | example must be an obvious placeholder, not a valid token |
| `refresh_token` | String | `"<jwt>"` | placeholder |

### `GoogleAuthResponse`
Google-login success (200). Same as `AuthTokensResponse` plus:
| Field | Type | Example | Notes |
|-------|------|---------|-------|
| `is_new_user` | bool | `false` | true when a new account was provisioned |

### `RefreshResponse`
Refresh success (200).
| Field | Type | Example |
|-------|------|---------|
| `access_token` | String | `"<jwt>"` |
| `refresh_token` | String | `"<jwt>"` |

### `StatusResponse`
Logout success (200).
| Field | Type | Example |
|-------|------|---------|
| `status` | String | `"logged_out"` |

### `HealthResponse`
Health (200).
| Field | Type | Example |
|-------|------|---------|
| `status` | String | `"ok"` |

### `ErrorResponse`
Shared error envelope for all non-2xx JSON responses (and, per research Decision 4, the
normalized logout 401).
| Field | Type | Example |
|-------|------|---------|
| `error` | String | `"Invalid username or password"` |

---

## Use-case result → wire DTO mapping (handler boundary)

The domain/application layer keeps its own result structs; handlers map them to DTOs.
No serde/schema derives are added to these internal types.

| Use-case result (internal) | Maps to DTO | Where |
|----------------------------|-------------|-------|
| `login_with_password::LoginResult` | `AuthTokensResponse` | `login_routes.rs` handler |
| `login_with_google::LoginWithGoogleResult` | `GoogleAuthResponse` | `oauth_callback.rs` handler |
| `refresh_token::RefreshResult` | `RefreshResponse` | `refresh_routes.rs` handler |
| logout `Ok(auth_method)` | `StatusResponse` | `logout_routes.rs` handler |
| `AuthError` variants | `ErrorResponse` (+ status code) | each handler's match arms |

---

## Security Scheme

| Attribute | Value |
|-----------|-------|
| Name | `bearer_jwt` |
| Type | HTTP, scheme = `bearer`, `bearerFormat = JWT` |
| Placement | `Authorization: Bearer <access_token>` header |
| Applied to | `logout` only (the sole endpoint gated by `AuthenticatedUser`) |
| Source of truth | `auth_middleware.rs:38-51` |

---

## Documented Endpoint Outcomes (status code matrix)

Each row becomes a `responses(...)` entry on the endpoint's `#[utoipa::path]`.

| Endpoint | Method | 200 | 401 | 422 | 429 | 500 | Auth |
|----------|--------|-----|-----|-----|-----|-----|------|
| `/api/auth/login/password` | POST | `AuthTokensResponse` | `ErrorResponse` (invalid creds) | `ErrorResponse` (missing fields) | `ErrorResponse` (rate limit) | `ErrorResponse` | none |
| `/api/auth/login/google` | POST | `GoogleAuthResponse` | `ErrorResponse` (verification failed) | `ErrorResponse` (missing token) | — | `ErrorResponse` | none |
| `/api/auth/logout` | POST | `StatusResponse` | `ErrorResponse` (missing/invalid Bearer — normalized) | — | — | `ErrorResponse` | **bearer_jwt** |
| `/api/auth/refresh` | POST | `RefreshResponse` | `ErrorResponse` (invalid/expired/verification/session) | `ErrorResponse` (missing token) | `ErrorResponse` (rate limit) | `ErrorResponse` | none |
| `/api/health` | GET | `HealthResponse` | — | — | — | — | none |

Notes:
- Logout also returns **400** ("Invalid session") for `SessionNotFound/Invalidated/Expired` at the handler level — document as 400 `ErrorResponse` in addition to the 401 auth-rejection.
- `/metrics` is intentionally **not** documented (research Decision 7).

---

## OpenAPI Document Metadata

| Attribute | Value |
|-----------|-------|
| OpenAPI version | 3.x (emitted by `utoipa` 5.x) |
| `info.title` | "App Home Services API" |
| `info.version` | crate version (`0.1.0`) |
| `info.description` | Modular monolith API — bounded contexts: Authentication, Profiles, Admin, Health |
| `paths` | the 5 documented endpoints above |
| `components.schemas` | all request + response DTOs above |
| `components.securitySchemes` | `bearer_jwt` |

---

## Consistency Sources

| Source | Role |
|--------|------|
| `ApiDoc::openapi()` (generated) | Authoritative for as-implemented shape |
| `specs/002-audit-security-hardening/contracts/*.md` | Complementary human narrative; consistency-checked (endpoints, methods, status codes) |
| `specs/001-user-auth/contracts/*.md` | Older narrative (login-password, login-google) |
