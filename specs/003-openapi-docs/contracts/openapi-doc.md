# Contract: OpenAPI Documentation Endpoints

**Feature**: `specs/003-openapi-docs` | **Date**: 2026-07-15

This contract defines the two new documentation surfaces and the guarantees the
generated specification must satisfy. It is complementary to the auth-endpoint
contracts under `specs/002-audit-security-hardening/contracts/`.

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
  - `paths` contains exactly the documented public endpoints:
    - `POST /api/auth/login/password`
    - `POST /api/auth/login/google`
    - `POST /api/auth/logout`
    - `POST /api/auth/refresh`
    - `GET /api/health`
  - `paths` does **not** contain `/metrics` (excluded by policy).
  - `components.schemas` includes: `PasswordLoginRequest`, `GoogleLoginRequest`,
    `LogoutRequest`, `RefreshTokenRequest`, `AuthTokensResponse`, `GoogleAuthResponse`,
    `RefreshResponse`, `StatusResponse`, `HealthResponse`, `ErrorResponse`.
  - `components.securitySchemes` includes `bearer_jwt` (HTTP bearer, format JWT).
  - The `logout` operation declares `security: [{ bearer_jwt: [] }]`; no other operation does.
  - Each operation lists all documented status codes for that endpoint (see `data-model.md` matrix).
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
  - Lists every documented endpoint with method, path, summary, request schema (where applicable),
    and response schemas.
  - Provides an "Authorize" affordance for the Bearer JWT scheme; the logout endpoint shows a
    padlock / required-auth indicator.
  - "Try it out" issues live requests to the running service and displays the actual status code
    and response body.

---

## Coverage Guarantee (FR-012 / SC-008)

An automated `cargo test` (`tests/openapi_coverage_test.rs`) MUST fail if:
- any endpoint in the documented set is missing from `ApiDoc::openapi()`, OR
- `ApiDoc` documents an endpoint not intended by policy (e.g. `/metrics` appears).

This prevents a new public endpoint from shipping undocumented.

---

## Consistency Guarantee (FR-011 / SC-007)

An automated `cargo test` (`tests/docs_contract_consistency_test.rs`) MUST compare the
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
