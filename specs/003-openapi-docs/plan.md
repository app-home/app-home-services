# Implementation Plan: OpenAPI & Swagger Documentation

**Branch**: `feat/openapi-swagger-docs` | **Date**: 2026-07-15 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/003-openapi-docs/spec.md`

## Summary

Add first-class, code-generated OpenAPI documentation to the existing Axum modular monolith and serve it both as a machine-readable document (`/api-docs/openapi.json`) and an interactive UI (`/swagger-ui`). The specification is generated from the implementation via `utoipa` annotations, rendered by `utoipa-swagger-ui`. All public endpoints get typed request/response schemas (replacing today's ad-hoc `serde_json::json!` responses with `#[derive(Serialize, ToSchema)]` DTOs), every status code is documented, and a Bearer JWT security scheme is registered for protected endpoints. The existing Markdown contracts under `specs/*/contracts` are preserved as complementary narrative, and an automated consistency + coverage check keeps the two aligned over time.

## Technical Context

**Language/Version**: Rust, Edition 2024 (per constitution)

**Primary Dependencies**: Existing — `axum` 0.7, `tokio`, `serde`, `serde_json`, `sqlx`, `jsonwebtoken`, `tower-http`. New — `utoipa` (OpenAPI generation via derive/proc-macros) and `utoipa-swagger-ui` (bundled Swagger UI + Axum integration). Both pinned to versions compatible with `axum` 0.7.

**Storage**: PostgreSQL via SQLx — **unchanged** by this feature (no migrations, no schema changes).

**Testing**: `cargo test` — unit tests (schema/DTO round-trips), integration tests (spec endpoint returns valid OpenAPI, coverage of all routes), and a consistency check test comparing the generated spec against the Markdown contracts.

**Target Platform**: Linux server (containerized; see `Containerfile` / `compose.yaml`).

**Project Type**: Modular Rust monolith with hexagonal architecture (bounded contexts).

**Performance Goals**: The spec document is generated once at startup (or lazily, cached) and served as a static value; serving `/api-docs/openapi.json` must be O(1) with no per-request generation cost. No measurable impact on existing auth endpoint latency.

**Constraints**:
- Documentation UI/spec must not weaken existing CORS/access posture (`main.rs:89-110`).
- Example/default values in the spec MUST NOT contain real secrets or valid tokens.
- Must compile with zero warnings; pass `cargo fmt` and `cargo clippy`.
- Response DTOs must live in the inbound adapter layer and must not leak into or from the domain.

**Scale/Scope**: 6 routed endpoints (4 auth + health + metrics), 4 request bodies, ~5 response body shapes plus a shared error shape. Documentation policy: document the 4 auth endpoints + `/api/health`; **exclude `/metrics`** (Prometheus text format, not JSON — consistent with `main.rs:118-127` intent).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Impact | Status |
|-----------|--------|--------|
| **I. Hexagonal Architecture (NON-NEGOTIABLE)** | OpenAPI annotations and response DTOs are strictly an **inbound adapter** concern. Domain and application layers are not modified and must not gain `utoipa`/serde-web dependencies. Dependency direction stays Adapters → Application → Domain. | ✅ PASS |
| **II. Domain-Driven Design** | New typed response DTOs represent HTTP wire shapes, not domain concepts; they live in `adapters/inbound` and map from use-case results. Domain entities remain free of serialization/schema derives. | ✅ PASS |
| **III. Rust-First Development** | Two new dependencies (`utoipa`, `utoipa-swagger-ui`) — justified below and permitted by the constitution's "Additional dependencies must be justified" clause. No `unsafe`. Errors stay explicit; no new `unwrap()`/`expect()` in request paths (spec is built from static data). Must pass fmt + clippy + zero warnings. | ✅ PASS (deps justified) |
| **IV. Test-First and Quality Gates (NON-NEGOTIABLE)** | Behavior defined before implementation: integration test asserts `/api-docs/openapi.json` is valid and covers all documented routes; a coverage guard fails on undocumented public endpoints; a consistency test compares spec vs. Markdown contracts. Tests authored before/with implementation. | ✅ PASS |
| **V. PostgreSQL Persistence Standards** | No database or schema changes. | ✅ PASS (N/A) |
| **VI. Observability and Reliability** | No change to logging/metrics/health behavior. `/metrics` remains as-is. | ✅ PASS |

**Dependency justification (Principle III / Technology Constraints)**: `utoipa` is the idiomatic, actively maintained OpenAPI generator for Axum 0.7 and satisfies FR-009 (generate-from-implementation) with compile-time derive macros, avoiding a separately hand-maintained spec. `utoipa-swagger-ui` provides the interactive UI (FR-002) with native Axum wiring. No lighter in-tree alternative exists that generates a spec from typed handlers; hand-writing the OpenAPI document would reintroduce exactly the drift this feature exists to eliminate.

**Result**: No violations. Complexity Tracking section intentionally omitted (nothing to justify beyond the two dependencies above).

**Post-Design Re-check (after Phase 1)**: Re-evaluated against the generated `research.md`, `data-model.md`, and `contracts/openapi-doc.md`. All DTOs remain confined to `adapters/inbound/` with mapping at the handler boundary (Hexagonal/DDD preserved); the only behavior change (logout 401 normalized to JSON `ErrorResponse`) is contained in the inbound adapter and flagged for reviewer sign-off; dependency versions resolved to axum-0.7-compatible `utoipa = "5"` / `utoipa-swagger-ui = "8"`; test-first honored via three contract tests; no DB/observability changes. **Gate still PASSES — no new violations introduced by the design.**

## Project Structure

### Documentation (this feature)

```text
specs/003-openapi-docs/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
│   └── openapi-doc.md   # Contract for the doc endpoints + expected spec shape
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created here)
```

### Source Code (repository root)

```text
src/
├── domain/                       # UNCHANGED (no schema derives leak here)
│   ├── entities/
│   └── errors.rs
├── application/                  # UNCHANGED
│   ├── ports/
│   └── use_cases/
├── adapters/
│   ├── inbound/
│   │   ├── login_routes.rs       # + ToSchema on PasswordLoginRequest; #[utoipa::path]; typed response DTO
│   │   ├── oauth_callback.rs      # + ToSchema on GoogleLoginRequest; #[utoipa::path]; typed response DTO
│   │   ├── logout_routes.rs       # + ToSchema on LogoutRequest; #[utoipa::path]; typed response DTO
│   │   ├── refresh_routes.rs      # + ToSchema on RefreshTokenRequest; #[utoipa::path]; typed response DTO
│   │   ├── auth_middleware.rs      # (documented as Bearer security; behavior unchanged)
│   │   ├── responses.rs           # NEW: shared response DTOs (AuthTokens, ErrorResponse, StatusResponse, HealthResponse)
│   │   └── api_doc.rs             # NEW: #[derive(OpenApi)] ApiDoc + Bearer security scheme modifier
│   └── outbound/                  # UNCHANGED
├── infrastructure/               # UNCHANGED
└── main.rs                       # + mount SwaggerUi + /api-docs/openapi.json; #[utoipa::path] on health_check

tests/
├── openapi_spec_test.rs          # NEW: spec served, valid OpenAPI, security scheme present
├── openapi_coverage_test.rs      # NEW: every routed public endpoint appears in the spec
└── docs_contract_consistency_test.rs  # NEW: spec vs specs/*/contracts alignment (endpoints/methods/status codes)

scripts/
└── (optional) check-docs-consistency  # Wraps the consistency test for CI/build workflow
```

**Structure Decision**: Modular Rust monolith following the existing hexagonal layout mandated by the constitution. All new code is confined to `src/adapters/inbound/` (DTOs + `ApiDoc`) and `src/main.rs` (route wiring), plus `tests/`. No changes to `domain/`, `application/`, `infrastructure/`, or the database. This keeps the OpenAPI concern entirely in the inbound adapter, preserving the inward dependency rule.

## Key Design Decisions (feed Phase 0 / Phase 1)

1. **Typed response DTOs replace `serde_json::json!`**: Introduce `AuthTokensResponse` (login/refresh), `GoogleAuthResponse` (adds `is_new_user`), `StatusResponse` (logout), `HealthResponse`, and a shared `ErrorResponse { error: String }`. This is required because `utoipa` needs typed schemas; it also improves type safety. Handlers change return shape but not status codes or JSON field names (backward compatible).
2. **Logout auth-rejection shape**: `auth_middleware.rs` currently returns plain-text `"Unauthorized"` (401), NOT JSON. The spec must document logout's 401 accurately (plain text) — or the rejection is refactored to emit `ErrorResponse` JSON for consistency. **Decision deferred to research.md** (documentation-only vs. small behavior-normalizing refactor); default recommendation is to normalize to JSON `ErrorResponse` for a consistent contract, noting it as a minor behavior change.
3. **`/metrics` excluded** from the OpenAPI document (Prometheus text format); `/api/health` included. Recorded per FR-015.
4. **CORS/exposure**: Swagger UI assets are served by the app; verify the configured-origins branch (`main.rs:96-109`, GET/POST only) does not block UI/spec GETs, and that adding the routes does not broaden access to auth endpoints.
5. **Consistency mechanism (FR-011/FR-012)**: A test-time check that (a) enumerates the router's public endpoints and asserts each is present in `ApiDoc` (coverage guard), and (b) parses the `specs/*/contracts/*.md` front matter/paths and compares endpoint+method+status-code sets against the generated spec. Runs under `cargo test` so it gates releases via the existing workflow.

## Phase 0 — Research (research.md)

Resolve: exact compatible `utoipa` + `utoipa-swagger-ui` versions for `axum` 0.7; the idiomatic `SwaggerUi` mounting pattern with `.with_state(...)`; how to declare a Bearer JWT `SecurityScheme` via an `OpenApi` `Modify` modifier; decision on the logout 401 plain-text vs JSON normalization; approach for parsing the Markdown contracts for the consistency check.

## Phase 1 — Design & Contracts

- **data-model.md**: Document the response/request DTOs (fields, types, required flags, non-sensitive examples), the security scheme, and the mapping from use-case result structs to wire DTOs.
- **contracts/openapi-doc.md**: Contract for `/api-docs/openapi.json` (returns valid OpenAPI 3.x JSON) and `/swagger-ui` (serves the UI), plus the expected documented endpoint set and the coverage/consistency guarantees.
- **quickstart.md**: How to run the service, open `/swagger-ui`, fetch `/api-docs/openapi.json`, validate it, and run the coverage/consistency tests.
- **Agent context**: Update the `AGENTS.md` SPECKIT block to point at this plan.

## Complexity Tracking

No constitution violations requiring justification. (Two new dependencies are justified in the Constitution Check above and permitted by the Technology Constraints clause.)
