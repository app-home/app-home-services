# Contract: OpenAPI Documentation Endpoints

**Feature**: `specs/003-openapi-docs` | **Date**: 2026-07-19

This contract defines the two documentation surfaces and the guarantees the
generated specification must satisfy. It is complementary to the endpoint
contracts under `specs/002-audit-security-hardening/contracts/`,
`specs/005-user-profiles/contracts/`, and `specs/006-admin/contracts/`.

---

## Endpoint: `GET /api-docs/openapi.json`

Serves the machine-readable OpenAPI document.

- **Method**: GET
- **Auth**: none
- **Request body**: none
- **Response 200**: `application/json` — a valid OpenAPI 3.x document.
- **Content guarantees**:
  - `openapi` version field present (3.x).
  - `info.title`, `info.version` populated.
  - `info.description` = `"App Home Services API"`.
  - `tags` includes `Authentication`, `Profiles`, `Admin`, and `Health` with descriptions.
  - `paths` contains all documented public endpoints (grouped by tag):
    - `POST /api/auth/login/password` (Authentication)
    - `POST /api/auth/login/google` (Authentication)
    - `POST /api/auth/logout` (Authentication)
    - `POST /api/auth/refresh` (Authentication)
    - `GET /api/health` (Health)
    - `GET /api/profile` (Profiles)
    - `PUT /api/profile` (Profiles)
    - `GET /api/admin/users` (Admin)
    - `GET /api/admin/users/{id}` (Admin)
    - `PUT /api/admin/users/{id}/role` (Admin)
  - `paths` does **not** contain `/metrics` (excluded by policy).
  - `components.schemas` includes: `PasswordLoginRequest`, `GoogleLoginRequest`,
    `LogoutRequest`, `RefreshTokenRequest`, `AuthTokensResponse`, `GoogleAuthResponse`,
    `RefreshResponse`, `StatusResponse`, `HealthResponse`, `ErrorResponse`,
    `ProfileResponse`, `UpdateProfileRequest`, `UserResponse`, `UpdateRoleRequest`.
  - `components.securitySchemes` includes `bearer_jwt` (HTTP bearer, format JWT).
  - Operations that require auth declare `security: [{ bearer_jwt: [] }]`:
    `POST /api/auth/logout`, all profile and admin endpoints.
  - Each operation lists all documented status codes for that endpoint (see individual
    contracts and `data-model.md` matrix).
  - No example/default value contains a real secret, credential, or valid token.

**Validation**: The returned document MUST pass a standard OpenAPI 3.x validator with zero
structural errors (SC-003) and MUST be importable by a standard client generator without manual
edits (SC-006).

---

## Endpoint: `GET /swagger-ui` (and sub-assets)

Serves the interactive Swagger UI that renders `/api-docs/openapi.json`.

- **Method**: GET
- **Auth**: none
- **Response 200**: `text/html` (UI shell) plus static assets served by `utoipa-swagger-ui`.
- **Behavior guarantees**:
  - Lists every documented endpoint grouped by tag (Authentication, Profiles, Admin, Health)
    with method, path, summary, request schema (where applicable), and response schemas.
  - Provides an "Authorize" affordance for the Bearer JWT scheme; protected endpoints show a
    padlock / required-auth indicator.
  - "Try it out" issues live requests to the running service and displays the actual status code
    and response body.

---

## Coverage Guarantee (FR-012 / SC-008)

An automated `cargo test` (`tests/openapi_coverage.rs`) MUST fail if:
- any endpoint in the documented set is missing from `ApiDoc::openapi()`, OR
- `ApiDoc` documents an endpoint not intended by policy (e.g. `/metrics` appears).

This prevents a new public endpoint from shipping undocumented.

---

## Consistency Guarantee (FR-011 / SC-007)

An automated `cargo test` (`tests/markdown_contract_consistency.rs`) MUST compare the
generated spec against the Markdown contracts under `specs/*/contracts/` and report divergence in:
- the set of endpoints,
- HTTP methods per endpoint,
- documented status codes per endpoint.

Divergence fails the test. The Markdown contracts remain the human-readable narrative
(FR-010) and are not deleted or auto-generated.

---

## Non-Functional Guarantees

- **Performance**: `ApiDoc::openapi()` is built once and served as static data; no per-request
  spec generation.
- **Exposure (FR-013)**: The doc routes are GET and served under the existing `CorsLayer` without
  broadening `allow_origin` / `allow_methods` for other routes.
- **Quality**: New code compiles with zero warnings and passes `cargo fmt` + `cargo clippy`.
