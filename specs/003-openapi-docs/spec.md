# Feature Specification: OpenAPI & Swagger Documentation

**Feature Branch**: `feat/openapi-swagger-docs`

**Created**: 2026-07-15

**Status**: Draft

**Input**: User description: "Add first-class OpenAPI documentation to the Axum service using utoipa and utoipa-swagger-ui while preserving the existing Markdown API contracts under specs/*/contracts as complementary documentation. Generate OpenAPI directly from the implementation, expose /api-docs/openapi.json and /swagger-ui, document all public endpoints with typed request and response schemas, register the Bearer JWT security scheme for protected endpoints, and ensure the generated specification accurately reflects the current API behavior, validation rules, status codes, and rate limiting. The specification should define how both documentation formats remain consistent over time, include architectural impact, affected modules, testing strategy, maintenance guidelines, and measurable acceptance criteria."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Explore and try the API interactively (Priority: P1)

An API consumer (front-end developer, integrator, or QA engineer) opens the service's interactive documentation in a browser, browses every available endpoint, reads the request and response shapes, understands which fields are required, sees the possible status codes, and issues a live request against a running instance without leaving the page.

**Why this priority**: This is the core value of the feature. Without an interactive, always-accurate rendering of the API, consumers must read source code or manually maintained Markdown, which is slow and drifts from reality. Delivering just this story already provides a usable, self-service documentation portal.

**Independent Test**: Start the service, navigate to the documentation UI, confirm every public endpoint is listed with its method, path, request body, response bodies, and status codes, then execute a login request from the UI and receive a valid response.

**Acceptance Scenarios**:

1. **Given** the service is running, **When** a consumer opens the interactive documentation page, **Then** all public endpoints are listed with their HTTP method, path, summary, request schema (where applicable), and response schemas.
2. **Given** the interactive documentation page is open, **When** a consumer expands the password-login endpoint and submits a valid request, **Then** the UI displays the real response body and status code returned by the service.
3. **Given** a consumer views a protected endpoint, **When** they inspect its documentation, **Then** the required Bearer token authorization is clearly indicated.
4. **Given** a consumer views any endpoint, **When** they inspect its documented outcomes, **Then** every possible status code (success and error) is listed with a description of when it occurs.

---

### User Story 2 - Consume a machine-readable specification (Priority: P1)

A tooling author or integrator retrieves a standard machine-readable API description from a stable endpoint and feeds it into client-generator tools, contract-testing tools, or API gateways.

**Why this priority**: The machine-readable artifact unlocks automation (SDK generation, contract validation, gateway import). It is the foundation the interactive UI itself renders from, so it must be correct and stable. It is co-equal P1 with the UI because both are direct deliverables and depend on the same underlying description.

**Independent Test**: Request the specification document from its published endpoint, confirm it is a valid OpenAPI document, and confirm every endpoint, request schema, response schema, and security scheme present in the running service appears in the document.

**Acceptance Scenarios**:

1. **Given** the service is running, **When** a consumer requests the specification document endpoint, **Then** a valid OpenAPI document is returned describing every public endpoint.
2. **Given** the specification document, **When** it is validated by a standard OpenAPI validator, **Then** it passes without structural errors.
3. **Given** the specification document, **When** a consumer inspects a protected endpoint, **Then** a Bearer JWT security scheme is declared and referenced by that endpoint.
4. **Given** the specification document, **When** it is imported into a standard client-generation tool, **Then** a client can be generated without manual editing of the document.

---

### User Story 3 - Keep documentation consistent over time (Priority: P2)

A maintainer changes an endpoint's request or response shape and needs the published documentation to reflect the change without a separate manual documentation step, while the human-authored Markdown contracts remain a complementary, reviewable narrative.

**Why this priority**: Documentation drift is the primary risk this feature exists to solve. Ensuring the generated description tracks the implementation—and defining how the Markdown contracts stay aligned—protects the long-term value. It is P2 because the initial value (US1, US2) can ship before the consistency safeguards are fully automated.

**Independent Test**: Change a documented request or response field in the implementation, rebuild, and confirm the generated specification and interactive UI reflect the change automatically; confirm a defined check flags any divergence between the generated specification and the Markdown contracts.

**Acceptance Scenarios**:

1. **Given** a maintainer changes a documented field in the implementation, **When** the service is rebuilt, **Then** the generated specification and interactive UI reflect the change with no manual documentation edit.
2. **Given** the Markdown contracts and the generated specification, **When** the consistency check runs, **Then** any divergence in endpoints, methods, or documented status codes is reported.
3. **Given** a new public endpoint is added, **When** the documentation coverage check runs, **Then** it fails if that endpoint is not documented.

---

### Edge Cases

- **Undocumented endpoint added**: When a new public endpoint is introduced but not annotated, the coverage safeguard must surface it rather than silently omit it from the documentation.
- **Rate-limited endpoint hit from the UI**: When a consumer triggers the rate limit while testing an endpoint from the UI, the documented rate-limit status code and its meaning must be visible so the response is understood rather than mistaken for a failure.
- **Protected endpoint without a token**: When a consumer invokes a protected endpoint from the UI without providing a Bearer token, the documented unauthorized outcome must be returned and explained.
- **Invalid or malformed request body**: When a consumer submits a body that fails validation, the documented validation-error outcome and its status code must be presented.
- **Documentation routes and cross-origin access**: When the documentation UI and specification are served, existing cross-origin and access policies must not unintentionally block them, nor unintentionally widen access to other endpoints.
- **Non-public/internal endpoints**: Operational endpoints (health, metrics) must be documented or explicitly excluded per a stated policy, never left ambiguous.
- **Sensitive values in examples**: Any example values shown in the documentation must not expose real secrets, tokens, or credentials.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST expose a machine-readable API specification document in the OpenAPI format at a stable, published endpoint.
- **FR-002**: The system MUST expose an interactive documentation user interface at a stable, published path that renders the specification and allows issuing live requests.
- **FR-003**: The specification MUST document every public endpoint of the service, including HTTP method, path, and a human-readable summary.
- **FR-004**: The specification MUST define typed request schemas for every endpoint that accepts a request body, including which fields are required and their data types.
- **FR-005**: The specification MUST define typed response schemas for every endpoint, for both successful and error outcomes.
- **FR-006**: The specification MUST enumerate every possible status code each endpoint can return, with a description of the condition that produces it (including success, validation error, unauthorized, and rate-limited outcomes).
- **FR-007**: The specification MUST declare a Bearer JWT security scheme and associate it with every endpoint that requires authentication.
- **FR-008**: The specification MUST accurately reflect current API behavior, including validation rules and rate-limiting behavior, so that documented outcomes match what a live request produces.
- **FR-009**: The specification MUST be generated from the implementation so that documented endpoints and schemas track the code without a separate manual authoring step.
- **FR-010**: The system MUST retain the existing Markdown API contracts under `specs/*/contracts` as complementary human-readable documentation and MUST NOT remove them as part of this feature.
- **FR-011**: The system MUST define and provide a repeatable check that verifies consistency between the generated specification and the Markdown contracts (at minimum: matching set of endpoints, methods, and documented status codes) and reports divergence.
- **FR-012**: The system MUST provide a documentation-coverage safeguard that fails when a public endpoint exists without a corresponding entry in `ApiDoc::paths(...)` (i.e., a `#[utoipa::path]` annotation aggregated into the `OpenApi` derive).
- **FR-013**: The interactive documentation UI and specification endpoint MUST remain reachable under the service's existing cross-origin and access policies without unintentionally broadening access to other endpoints.
- **FR-014**: Example and default values shown in the documentation MUST NOT contain real secrets, credentials, or valid tokens.
- **FR-015**: The system MUST state an explicit policy for operational endpoints (health, metrics): whether each is documented in the specification or intentionally excluded.
- **FR-016**: Maintenance guidelines MUST be documented describing how to add or change endpoint documentation, how to update the Markdown contracts, and how the consistency and coverage checks are run.

### Key Entities *(include if feature involves data)*

- **API Specification Document**: The generated machine-readable description of the API. Attributes: format/version, list of documented endpoints, reusable schemas, declared security schemes. Relationships: rendered by the interactive UI; validated against the Markdown contracts.
- **Documented Endpoint**: A single public operation. Attributes: method, path, summary, required auth indicator, request schema reference, set of response outcomes (status code + schema + description). Relationships: belongs to the specification; may correspond to a Markdown contract file.
- **Request/Response Schema**: A typed representation of a request or response body. Attributes: field names, types, required/optional flags, example values (non-sensitive). Relationships: referenced by documented endpoints.
- **Security Scheme**: The declared authentication method (Bearer JWT). Attributes: type, token placement, description. Relationships: referenced by protected documented endpoints.
- **Markdown Contract**: An existing human-authored contract file under `specs/*/contracts`. Attributes: endpoint, described request/response, status codes. Relationships: complementary to, and consistency-checked against, the generated specification.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the service's public endpoints appear in the generated specification with method, path, request schema (where applicable), and response schemas.
- **SC-002**: 100% of endpoints that require authentication are marked as requiring Bearer JWT authorization in the specification and interactive UI.
- **SC-003**: The generated specification passes a standard OpenAPI validator with zero structural errors.
- **SC-004**: Every documented status code shown for an endpoint can be reproduced by a corresponding live request (100% of documented outcomes are observable), covering at minimum success, validation error, unauthorized, and rate-limited cases where applicable.
- **SC-005**: A new API consumer can locate an endpoint, understand its request and response, and successfully issue a working request from the interactive UI in under 5 minutes without reading source code.
- **SC-006**: A client can be generated from the specification using a standard generator tool with zero manual edits to the specification document.
- **SC-007**: The consistency check between the generated specification and the Markdown contracts reports zero unexplained divergences in endpoints, methods, and documented status codes.
- **SC-008**: When a public endpoint is added without documentation, the coverage safeguard fails 100% of the time (no undocumented public endpoint can pass unnoticed).
- **SC-009**: No example or default value in the published documentation contains a real secret, credential, or valid token (verified by review, zero occurrences).

## Assumptions

- The current public API surface consists of the authentication endpoints (password login, Google login, logout, refresh) plus operational endpoints (health, metrics); the documentation policy for operational endpoints will be stated explicitly per FR-015. The recommended default is to document `health` and exclude `metrics` (Prometheus scrape format, not JSON), unless maintainers decide otherwise.
- "Public endpoints" means endpoints intended for external consumers; purely internal or operational endpoints may be excluded under a stated policy.
- The interactive UI is intended for development, testing, and integration use; whether it is enabled in production is a deployment decision and is out of scope for this specification beyond ensuring it can be served.
- The Markdown contracts under `specs/*/contracts` remain the authoritative human-readable narrative and source-of-truth for intended behavior; the generated specification is authoritative for the as-implemented shape.
- Existing authentication (Bearer JWT via the `Authorization` header) and rate-limiting behavior are unchanged by this feature **except for the logout 401 response, which is normalized from plain text to JSON `ErrorResponse` for contract consistency (see research.md Decision 4 — requires reviewer sign-off)**; the feature documents them rather than altering them.
- The published paths for the specification and interactive UI default to `/api-docs/openapi.json` and `/swagger-ui` respectively, per the user request.
- Consistency and coverage checks are expected to run as part of the existing build/test workflow so divergence is caught before release.
