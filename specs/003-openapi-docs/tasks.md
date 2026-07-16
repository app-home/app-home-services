# Tasks: OpenAPI & Swagger Documentation

**Input**: Design documents from `specs/003-openapi-docs/`
**Prerequisites**: `plan.md` (required), `spec.md` (required for user stories), `research.md`, `data-model.md`, `contracts/openapi-doc.md`, `quickstart.md`
**Tests**: The spec defines measurable acceptance criteria and the constitution mandates test-first (Principle IV). Test tasks are included and MUST be authored to fail before their corresponding implementation.

**Organization**: Tasks are grouped by user story (US1/HTTP-context-driven stories) so each can be implemented and tested independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

Single Rust project: source under `src/`, integration tests under `tests/` (project root).

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add the documentation-generation dependencies and confirm the build still works.

- [X] T001 Review `Cargo.toml` and confirm current state at `Cargo.toml` (record baseline of existing dependencies)
- [X] T002 Add `utoipa = "5"` dependency to `[dependencies]` in `Cargo.toml`
- [X] T003 Add `utoipa-swagger-ui = { version = "8", features = ["axum"] }` dependency to `[dependencies]` in `Cargo.toml`
- [X] T004 Run `cargo build` and `cargo clippy --all-targets -- -D warnings` to confirm new deps compile clean

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Build the shared infrastructure all user stories depend on — the response DTOs, the `ApiDoc` aggregator, the Swagger UI mount, and the normalized 401 shape. **No user story work can begin until this phase is complete.**

- [X] T005 Create `src/adapters/inbound/responses.rs` with `#[derive(Serialize, ToSchema)]` structs: `AuthTokensResponse`, `GoogleAuthResponse`, `RefreshResponse`, `StatusResponse`, `HealthResponse`, `ErrorResponse` — field names preserve current `serde_json::json!` output exactly per `data-model.md`
- [X] T006 [P] Refactor `src/adapters/inbound/login_routes.rs` (line 16) to add `#[derive(Deserialize, ToSchema)]` and non-sensitive `#[schema(example = …)]` values on `PasswordLoginRequest`
- [X] T007 [P] Refactor `src/adapters/inbound/oauth_callback.rs` (line 9) to add `ToSchema` + non-sensitive examples on `GoogleLoginRequest`
- [X] T008 [P] Refactor `src/adapters/inbound/logout_routes.rs` (line 11) to add `ToSchema` + non-sensitive examples on `LogoutRequest`
- [X] T009 [P] Refactor `src/adapters/inbound/refresh_routes.rs` (line 17) to add `ToSchema` + non-sensitive examples on `RefreshTokenRequest`
- [X] T009b [P] Add `#[schema(min_length = 1)]` and `#[schema(example = ...)]` annotations to all request DTO fields in `login_routes.rs`, `oauth_callback.rs`, `logout_routes.rs`, `refresh_routes.rs` to document validation rules (FR-008)
- [X] T010 Replace inline `serde_json::json!` in `src/adapters/inbound/login_routes.rs::login_password_handler` with the typed `AuthTokensResponse`/`ErrorResponse` DTOs from `responses.rs` (preserve status codes 200/401/422/429/500)
- [X] T011 [P] Replace inline `serde_json::json!` in `src/adapters/inbound/oauth_callback.rs::login_google_handler` with `GoogleAuthResponse`/`ErrorResponse` (status codes 200/401/422/500)
- [X] T012 [P] Replace inline `serde_json::json!` in `src/adapters/inbound/logout_routes.rs::logout_handler` with `StatusResponse`/`ErrorResponse` (status codes 200/400/500)
- [X] T013 [P] Replace inline `serde_json::json!` in `src/adapters/inbound/refresh_routes.rs::refresh_token_handler` with `RefreshResponse`/`ErrorResponse` (status codes 200/401/422/429/500)
- [X] T014 [P] Replace inline `serde_json::json!` in `src/main.rs::health_check` (line 180) with `HealthResponse`
- [X] T015 Normalize `AuthErrorResponse::into_response` in `src/adapters/inbound/auth_middleware.rs` (line 17) to emit JSON `ErrorResponse` instead of plain-text `"Unauthorized"` (research Decision 4 — contained behavior change in inbound adapter)
- [X] T015b [US3] Obtain reviewer sign-off on logout 401 JSON normalization (record in PR description)
- [X] T016 Create `src/adapters/inbound/api_doc.rs` with `pub struct ApiDoc` deriving `utoipa::OpenApi`; add `paths(…)` listing every `#[utoipa::path]` from handlers, `components(schemas(…))` listing every DTO from `responses.rs`, and a `Modify` impl appending the `bearer_jwt` HTTP Bearer security scheme (format JWT) per `data-model.md`
- [X] T017 Wire `SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi())` into the router in `src/main.rs` (around line 112) via `.merge(…)` before `.layer(cors)`; verify GET reachability under the configured-origins branch (`main.rs:96-109`, methods GET/POST) without broadening CORS for other routes
- [X] T018 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo build` to verify the foundation compiles cleanly with zero warnings

**Checkpoint**: Foundation ready — service still serves all auth endpoints with identical wire compatibility (request/response field names & status codes preserved) AND the new `/api-docs/openapi.json` + `/swagger-ui` routes are served. User story implementation can now begin.

---

## Phase 3: User Story 1 — Explore and try the API interactively (Priority: P1) 🎯 MVP

**Goal**: A consumer can open `/swagger-ui` and see every public endpoint with its schemas and status codes, then issue live "Try it out" requests.

**Independent Test**: Start the service, open `http://localhost:3000/swagger-ui` in a browser, confirm 5 endpoints are listed (4 auth + health), with `/metrics` absent; execute the password-login "Try it out" with seeded credentials and observe a 200 response matching `AuthTokensResponse`; confirm the logout endpoint shows a required-auth padlock.

### Tests for User Story 1 (write first, FAIL before annotations are added)

- [X] T019 [P] [US1] Create integration test `tests/openapi_spec_served.rs` asserting `GET /api-docs/openapi.json` returns 200 with JSON containing `openapi`, `info`, `paths`, `components`
- [X] T020 [P] [US1] Add to `tests/openapi_spec_served.rs` assertion that `/api-docs/openapi.json` includes the `bearer_jwt` security scheme and lists the `login-password`, `login-google`, `logout`, `refresh`, `health` paths; `metrics` path absent
- [X] T021 [P] [US1] Create unit test `tests/responses_serde.rs` asserting each DTO in `src/adapters/inbound/responses.rs` round-trips via `serde_json` with the exact field names captured in `data-model.md` placeholders

### Implementation for User Story 1

- [X] T022 [P] [US1] Annotate `login_password_handler` in `src/adapters/inbound/login_routes.rs` with `#[utoipa::path(post, path = "/api/auth/login/password", ..., responses(200, 401, 422, 429, 500), body = PasswordLoginRequest)]`
- [X] T023 [P] [US1] Annotate `login_google_handler` in `src/adapters/inbound/oauth_callback.rs` with `#[utoipa::path(post, path = "/api/auth/login/google", ..., responses(200, 401, 422, 500), body = GoogleLoginRequest)]`
- [X] T024 [P] [US1] Annotate `logout_handler` in `src/adapters/inbound/logout_routes.rs` with `#[utoipa::path(post, path = "/api/auth/logout", security(("bearer_jwt" = [])), responses(200, 400, 401, 500), body = LogoutRequest)]`
- [X] T025 [P] [US1] Annotate `refresh_token_handler` in `src/adapters/inbound/refresh_routes.rs` with `#[utoipa::path(post, path = "/api/auth/refresh", ..., responses(200, 401, 422, 429, 500), body = RefreshTokenRequest)]`
- [X] T026 [P] [US1] Annotate `health_check` in `src/main.rs` with `#[utoipa::path(get, path = "/api/health", ..., responses(200))]`
- [X] T027 [US1] Update `ApiDoc::paths(...)` in `src/adapters/inbound/api_doc.rs` to list the six `__path_*` constants the annotations generate (depends on T022–T026)
- [X] T028 [US1] Run `cargo test` and confirm T019–T021 now pass; then `cargo run` and manually verify `/swagger-ui` shows the 5 endpoints with the expected schemas and the logout padlock

**Checkpoint**: US1 fully functional and testable. A browser at `/swagger-ui` lists every documented endpoint with schemas, status codes, and the Bearer padlock on logout.

---

## Phase 4: User Story 2 — Consume a machine-readable specification (Priority: P1)

**Goal**: Tooling authors can fetch a valid, complete OpenAPI 3.x document from a stable endpoint and feed it into validators / client generators.

**Independent Test**: Validate the JSON returned by `GET /api-docs/openapi.json` against a standard OpenAPI validator with zero structural errors; generate a client successfully.

### Tests for User Story 2 (write first, FAIL before relevant work)

- [X] T029 [P] [US2] Create `tests/openapi_validity.rs` containing a single integration test `openapi_passes_structural_validation` that fetches `/api-docs/openapi.json` and runs it through a programmatic OpenAPI-3.x validator (assert at minimum: top-level required keys, well-formed `paths[method]`, components/schemas are objects) — fails today
- [X] T030 [P] [US2] Add test `openapi_security_scheme_well_formed` in `tests/openapi_validity.rs` asserting `components.securitySchemes.bearer_jwt.{type,bearerFormat}` equals `{"http", "JWT"}` and `scheme = "bearer"`

### Implementation for User Story 2

- [X] T031 [US2] Add any missing schema-references indicated by failing validator output (e.g. ensure every DTO used in `responses(...)` is present in `ApiDoc::components.schemas(...)`) and re-run T029/T030 to confirm green
- [X] T032 [US2] Manually confirm `cargo test` is green and `/api-docs/openapi.json` lints clean via an external validator (record in commit message)
- [X] T032b [US2] Add test `tests/openapi_status_codes_observable.rs` that hits each endpoint and asserts the documented status codes (200, 401, 422, 429, 500 per endpoint) are reproducible; document any status code that is impractical to automate (e.g. rate-limit 429 due to timing) with a note in the test

**Checkpoint**: US2 fully functional. The spec document passes programmatic structural validation AND a human-validated lint check.

---

## Phase 5: User Story 3 — Keep documentation consistent over time (Priority: P2)

**Goal**: Adding or changing an endpoint is reflected in the published documentation without a manual step; divergence between the generated spec and the Markdown contracts is detected automatically.

**Independent Test**: Change a request/response field in the implementation and rebuild; confirm the spec reflects the change; add a new public endpoint to `src/main.rs` without documenting it and confirm the coverage test fails.

### Tests for User Story 3 (write first, FAIL before guarantees land)

- [X] T033 [P] [US3] Create `tests/openapi_coverage.rs` defining a `DOCUMENTED_PATH_METHODS` constant (the explicit set: `login_password POST`, `login_google POST`, `logout POST`, `refresh POST`, `health GET`); add test `coverage_matches_expected_set` that fetches the spec, normalizes `paths`, and asserts every `(method,path)` in the documented set is present AND `/metrics` is absent — fails if drift is introduced
- [X] T034 [P] [US3] Add `tests/markdown_contract_consistency.rs` using the same expected set; add test `markdown_documented_set_present` asserting the parsed `specs/*/contracts/*.md` files contain, at minimum, the matching path/method for each documented endpoint (iterate over all glob matches)
- [X] T035 [P] [US3] Add to `tests/openapi_coverage.rs` test `coverage_fails_on_undocumented_endpoint` (compile-time): re-declare an additional `__path_undocumented` constant plus a temporary `#[utoipa::path]` annotation and assert the test logic catches it (remove the annotated temporary after the test confirms detection logic)

### Implementation for User Story 3

- [X] T036 [US3] Implement the spec-fetch + set-comparison logic for coverage in `tests/openapi_coverage.rs` using `core::panic!` with diff messages on mismatch (FR-012 / SC-008)
- [X] T037 [US3] Implement minimal Markdown front-matter / heading parsing for `specs/002-audit-security-hardening/contracts/*.md` in `tests/markdown_contract_consistency.rs` to extract `(method, path)` per file and compare against the documented set (FR-011 / SC-007); ignore unknown fields — only endpoints/methods/status-code coverage is in scope
- [X] T038 [US3] Run `cargo test`; T033–T035 must pass. Quickstart step 7 is now end-to-end runnable
- [X] T039 [US3] Write `specs/003-openapi-docs/maintenance.md` listing maintenance guidelines (FR-016): how to add or change endpoint documentation, how to update Markdown contracts, and how `cargo test` enforces coverage + consistency

**Checkpoint**: All 3 user stories independently functional. Drift on endpoints, methods, or status codes produces a red test; the published `/swagger-ui` and `/api-docs/openapi.json` remain the only authoritative hand-free artifact, with Markdown contracts as complementary narrative.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final quality gates and observability of the doc feature per FR-013, FR-014, FR-015, and the constitution's quality bar.

- [X] T040 [P] Audit every `ToSchema` and `#[utoipa::path]` annotation for non-sensitive example values (FR-014): no real secrets, real tokens, or valid credentials — replace any with obvious placeholders
- [X] T041 Confirm `/metrics` is not present in `ApiDoc::paths(...)`, recorded explicitly as the documented operational-endpoint policy for `metrics` (FR-015)
- [X] T042 Verify CORS: launch the service with non-empty `CORS_ALLOWED_ORIGINS`, `curl -H "Origin: <configured>" /api-docs/openapi.json` and `/swagger-ui` from that origin return correct CORS headers; auth routes unchanged (FR-013)
- [X] T043 Run `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` and confirm zero warnings
- [X] T044 Run `quickstart.md` end-to-end (steps 1–8) and confirm every expected outcome holds
- [X] T045 [P] Document the operational-endpoint policy in `specs/003-openapi-docs/maintenance.md` (FR-015): `/api/health` documented; `/metrics` intentionally excluded
- [X] T046 [P] Verify generated `openapi.json` passes an external linter (e.g. `npx @redocly/cli lint openapi.json`) with zero structural errors (SC-003)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup (T002–T004 must finish before Phase 2 begins) — **BLOCKS all user stories**
- **User Stories (Phases 3–5)**: All depend on Foundational (Phase 2) completion
  - Stories can proceed in parallel (different test files)
  - Or sequentially in priority order (US1 → US2 → US3)
- **Polish (Phase 6)**: Depends on all user stories being complete

### User Story Dependencies

- **US1 (P1)**: Depends only on Foundational phase. No dependencies on US2/US3.
- **US2 (P1)**: Depends only on Foundational phase. Builds on US1's annotations but is independently testable.
- **US3 (P2)**: Depends only on Foundational phase. May run after US1/US2 to reuse already-shipped annotations.

### Within Each User Story

- Tests MUST be written and FAIL before implementation (TDD)
- DTOs and request annotations (Phase 2) before path annotations (per-story)
- Path annotations before `ApiDoc::paths(...)` aggregation
- Aggregation before test validation

### Parallel Opportunities

- T002/T003 (add deps) parallelizable after T001
- T006–T009 all parallelizable — different files
- T010–T014 all parallelizable after T005 — different handler files
- T019/T020/T021 (test files) parallelizable after Phase 2 complete
- T022–T026 (path annotations) parallelizable — different files
- T029/T030 (validator test additions) parallelizable
- T033/T034 (coverage/contract tests) parallelizable
- T040/T045/T046 (polish audits) parallelizable

### Critical Path

T001 → T002 + T003 → T004 → T005 → T006–T009 → T016 (ApiDoc) → T017 (mount) → T019–T021 → T022–T026 → T027 → T028 (US1 shipped) → T031 → T032 (US2 shipped) → T036–T038 → T043/T046 (Polish).

---

## Parallel Examples

### Setup phase (after T001)

```bash
# T002 and T003 edit the same Cargo.toml — sequence, not parallel
Task: "Add utoipa to Cargo.toml"
Task: "Add utoipa-swagger-ui to Cargo.toml"
```

### Foundational — request-body annotations

```bash
Task: "Add ToSchema to PasswordLoginRequest in login_routes.rs"
Task: "Add ToSchema to GoogleLoginRequest in oauth_callback.rs"
Task: "Add ToSchema to LogoutRequest in logout_routes.rs"
Task: "Add ToSchema to RefreshTokenRequest in refresh_routes.rs"
```

### US1 — endpoint annotations

```bash
Task: "Annotate login_password_handler with #[utoipa::path] in login_routes.rs"
Task: "Annotate login_google_handler with #[utoipa::path] in oauth_callback.rs"
Task: "Annotate logout_handler with #[utoipa::path] (with bearer_jwt security) in logout_routes.rs"
Task: "Annotate refresh_token_handler with #[utoipa::path] in refresh_routes.rs"
Task: "Annotate health_check with #[utoipa::path] in main.rs"
```

### US3 — drift guard tests

```bash
Task: "Write coverage test in tests/openapi_coverage.rs"
Task: "Write Markdown contract consistency test in tests/markdown_contract_consistency.rs"
```

---

## Implementation Strategy

### MVP First (US1 only)

1. Phase 1 — Setup (add deps)
2. Phase 2 — Foundational (DTOs + ApiDoc + mount Swagger UI). **At this point, `/swagger-ui` and `/api-docs/openapi.json` already exist; DTOs and annotations complete.**
3. Phase 3 — US1 (annotations + first failing-then-green tests)
4. **STOP and VALIDATE**: Run `cargo test`, manually open `/swagger-ui`, exercise "Try it out"
5. US1 ships as the MVP

### Incremental Delivery

1. Setup + Foundational → service serves `/swagger-ui` and `/api-docs/openapi.json` (DTOs in place)
2. US1 → endpoint annotations appear in spec + UI; logout padlock works (MVP)
3. US2 → spec validated programmatically; safe for client generation
4. US3 → drift guards prevent future regressions
5. Polish → quality gates, CORS check, lint, quickstart

### Constitution Compliance Strategy

- **Principle I (Hexagonal)**: All new types live in `src/adapters/inbound/`; domain/application untouched
- **Principle III (Rust-First)**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, zero warnings
- **Principle IV (Test-First)**: T019/T020/T021/T029/T033/T034/T035 each authored to FAIL first
- **Principle VI (Observability)**: Unchanged; `/metrics` excluded from spec by policy

---

## Notes

- Each annotated handler exposes a `__path_<name>` constant that `ApiDoc::paths(...)` aggregates — keep the names predictable (and listed in T027's plan)
- `[P]` tasks touch different files; sequential tasks `cargo build` to check at logical checkpoints (T004, T018, T028, T032, T038)
- Tests are mandatory per Principle IV. **Do not skip any of T019/T020/T021/T029/T030/T033/T034**
- Commit logical groups: Setup / Foundation / each user story phase / Polish
- The single intentional behavior change is the logout 401 normalization (T015). It is contained to `auth_middleware.rs::AuthErrorResponse::into_response` and recorded in `research.md` Decision 4 and `contracts/openapi-doc.md`. Surface in PR description for reviewer sign-off.
