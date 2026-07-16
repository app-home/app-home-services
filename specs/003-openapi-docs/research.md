# Phase 0 Research: OpenAPI & Swagger Documentation

**Feature**: `specs/003-openapi-docs` | **Date**: 2026-07-15

This document resolves the open questions from the Technical Context in `plan.md`.

---

## Decision 1: OpenAPI generation & UI crates + versions

**Decision**: Use `utoipa = "5"` and `utoipa-swagger-ui = "8"` (specifically `utoipa-swagger-ui` 8.1.0 with the `axum` feature).

**Rationale**:
- The project uses `axum = "0.7"`. Dependency inspection on crates.io shows:
  - `utoipa-swagger-ui` **9.x requires axum ^0.8** — incompatible with this project without an axum upgrade.
  - `utoipa-swagger-ui` **8.0.3 / 8.1.0 require axum ^0.7 and utoipa ^5.0.0** — the correct match.
- `utoipa` 5.x is the current major line paired with swagger-ui 8.x and provides the `#[derive(OpenApi)]`, `#[derive(ToSchema)]`, and `#[utoipa::path]` macros needed for generate-from-code (FR-009).
- Staying on axum 0.7 avoids an unrelated, higher-risk framework upgrade inside a documentation feature.

**Alternatives considered**:
- **Upgrade to axum 0.8 + utoipa-swagger-ui 9.x**: Rejected for this feature — pulls a breaking framework migration into a docs task, expanding blast radius and violating "prefer simple solutions that satisfy current requirements" (constitution governance).
- **`aide`**: Viable OpenAPI generator, but `utoipa` is more idiomatic for annotation-driven Axum handlers and pairs directly with a bundled Swagger UI crate; fewer moving parts.
- **Hand-written `openapi.json` served statically**: Rejected — reintroduces the drift this feature exists to eliminate (fails FR-009).
- **`utoipa-axum` (router integration crate)**: Optional convenience for auto-collecting paths; not required for 6 endpoints. Keep the explicit `paths(...)` list in `ApiDoc` for clarity and to make the coverage guard meaningful. May revisit in tasks if boilerplate grows.

**Cargo.toml additions**:
```toml
utoipa = "5"
utoipa-swagger-ui = { version = "8", features = ["axum"] }
```

---

## Decision 2: Swagger UI + spec endpoint mounting pattern

**Decision**: Mount via `SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi())` merged into the existing `axum::Router` in `main.rs`, before `.with_state(state)` / `.layer(cors)`.

**Rationale**:
- `utoipa-swagger-ui`'s `SwaggerUi` implements the Axum integration when the `axum` feature is enabled; `.merge()` composes it with the existing routes without disturbing them.
- Serving the spec at `/api-docs/openapi.json` and UI at `/swagger-ui` matches the paths mandated by the user request and the spec Assumptions.
- The generated `ApiDoc::openapi()` value is built once and cloned into the router — O(1) serving, no per-request generation (satisfies the performance constraint).

**Alternatives considered**:
- Manually adding a `get()` route returning `Json(ApiDoc::openapi())`: works for the JSON endpoint but still needs the UI crate for `/swagger-ui`; `SwaggerUi.url(...)` already wires both, so prefer it.

**Open item for implementation**: Confirm `SwaggerUi` merges cleanly with a `Router<AppState>` state type; if a state-type mismatch arises, wrap with the crate's documented state-agnostic merge (verified during tasks).

---

## Decision 3: Bearer JWT security scheme declaration

**Decision**: Register an HTTP Bearer security scheme named `bearer_jwt` via a `Modify` implementation attached to `#[derive(OpenApi)]` (`modifiers(&SecurityAddon)`), and reference it with `security(("bearer_jwt" = []))` on the `#[utoipa::path]` of protected endpoints (currently only `logout`).

**Rationale**:
- Matches the real auth mechanism in `auth_middleware.rs:38-51`: `Authorization: Bearer <token>` validated as a JWT access token.
- A `Modify` modifier is the `utoipa` 5.x idiom for injecting a `SecurityScheme::Http(HttpBuilder ... HttpAuthScheme::Bearer)` with `bearer_format = "JWT"`.
- Only endpoints that actually require the token are marked, so the UI's "Authorize" affordance and the spec's `security` requirements are accurate (satisfies FR-007, SC-002).

**Alternatives considered**:
- Global `security` on the whole `OpenApi`: Rejected — would incorrectly imply login/refresh/health need a token.

---

## Decision 4: Logout 401 response shape (documentation vs. normalization)

**Decision**: **Normalize** the unauthorized rejection to a JSON `ErrorResponse { error: String }` so every endpoint shares one error contract, and document it as such. Record it as a minor, intentional behavior change.

**Context**: `auth_middleware.rs:17-21` currently returns plain-text `"Unauthorized"` with status 401, whereas all handler-level errors return JSON `{"error": "..."}`. Documenting the API honestly (FR-008) would otherwise require describing logout's 401 as `text/plain`, inconsistent with every other error.

**Rationale**:
- A single, uniform error schema is far cleaner for generated clients (SC-006) and for the interactive UI.
- The change is small and contained to the inbound adapter (`AuthErrorResponse::into_response`).
- It improves the contract without altering status codes or success shapes.

**Alternatives considered**:
- **Document as-is (plain text)**: Zero behavior change, but yields an inconsistent, harder-to-consume contract and a special-case schema. Rejected as the default; still acceptable if reviewers want strictly zero behavior change — the tasks phase will flag it for explicit sign-off.

**Impact**: Update `AuthErrorResponse` to emit `(StatusCode::UNAUTHORIZED, Json(ErrorResponse { error: "Unauthorized".into() }))`. Add/adjust a test asserting the JSON 401 shape. No change to when 401 occurs.

---

## Decision 5: Response DTOs replacing `serde_json::json!`

**Decision**: Introduce typed DTOs in a new `src/adapters/inbound/responses.rs`, each deriving `Serialize` + `utoipa::ToSchema`, and refactor handlers to return them. Field names and values are preserved exactly to keep the wire contract backward compatible.

DTOs and their current JSON sources:
| DTO | Fields (preserved names) | Replaces |
|-----|--------------------------|----------|
| `AuthTokensResponse` | `status: String` (="authenticated"), `user_id: String`, `access_token: String`, `refresh_token: String` | password login 200 (`login_routes.rs:115`) |
| `GoogleAuthResponse` | above + `is_new_user: bool` | google login 200 (`oauth_callback.rs:57`) |
| `RefreshResponse` | `access_token: String`, `refresh_token: String` | refresh 200 (`refresh_routes.rs:76`) |
| `StatusResponse` | `status: String` (="logged_out") | logout 200 (`logout_routes.rs:46`) |
| `HealthResponse` | `status: String` (="ok") | health 200 (`main.rs:181`) |
| `ErrorResponse` | `error: String` | all `{"error": ...}` responses |

**Rationale**: `utoipa` needs concrete types to derive schemas; typed DTOs also add compile-time safety and remove stringly-typed JSON. Keeping names/values identical means no client-visible change to success responses.

**Alternatives considered**:
- Keep `serde_json::json!` and hand-author schemas in `responses(...)`: Rejected — duplicates the shape in two places (drift risk), verbose, and loses type checking.

**Constitution note**: These DTOs are HTTP wire types and live in `adapters/inbound` only. Use-case result structs (`LoginResult`, etc.) are mapped into them at the handler boundary — the domain gains no serde/schema derives (Principle I & II preserved).

---

## Decision 6: Consistency & coverage checks (FR-011, FR-012)

**Decision**: Implement two `cargo test` integration tests so they run in the existing workflow:

1. **Coverage guard** (`tests/openapi_coverage_test.rs`): Assert every public endpoint the service documents by policy is present in `ApiDoc::openapi()`. Maintain an explicit expected-endpoint set (the 4 auth endpoints + `/api/health`); the test fails if the generated spec is missing any, or contains an undocumented one. Because `/metrics` is excluded by policy (Decision 7), it is asserted absent.
2. **Contract consistency** (`tests/docs_contract_consistency_test.rs`): Parse the `specs/*/contracts/*.md` files to extract each endpoint's method, path, and documented status codes, then compare those sets against the generated spec. Report any divergence (endpoint/method/status-code mismatch). The most complete contract set lives in `specs/002-audit-security-hardening/contracts/`.

**Rationale**: Running as tests means CI/`cargo test` gates releases (Principle IV), catching drift before merge (SC-007, SC-008) without new external tooling.

**Alternatives considered**:
- External CI script diffing generated JSON against a checked-in golden file: Adds a maintenance artifact and a non-Rust tool. A `cargo test` keeps it in-language and in-workflow. An optional wrapper `scripts/` entry may be added for convenience but the test is authoritative.
- Auto-generating Markdown from the spec: Rejected — the Markdown contracts are intentionally human narrative (complementary per FR-010), not a generated artifact.

---

## Decision 7: Operational endpoint documentation policy (FR-015)

**Decision**: Document `/api/health` in the OpenAPI spec (`HealthResponse`, 200). **Exclude `/metrics`** from the spec.

**Rationale**: `/metrics` emits Prometheus text exposition format, not JSON, and is intended for in-cluster scraping only (`main.rs:118-127`). Modeling it in OpenAPI would misrepresent it and add no consumer value. `/api/health` is a normal JSON endpoint consumers may use, so it is documented.

**Alternatives considered**:
- Document `/metrics` with a `text/plain` response: Rejected — low value, and its exposure is deliberately network-scoped.

---

## Decision 8: CORS / exposure impact (FR-013)

**Decision**: Mount the Swagger UI and spec routes and rely on the existing `CorsLayer`; verify during implementation that (a) the configured-origins branch (`main.rs:96-109`, methods GET/POST) permits GET to `/swagger-ui` and `/api-docs/openapi.json`, and (b) same-origin (no-origins) mode still serves the UI to a browser on the same origin. Do not broaden `allow_methods`/`allow_origin` for other routes.

**Rationale**: Swagger UI and the spec are GET requests; the existing GET allowance covers cross-origin reads where origins are configured, and same-origin browsing needs no CORS grant. No security posture change is required (satisfies FR-013).

**Open item for implementation**: If the "Try it out" feature must issue cross-origin requests to the auth endpoints from a differently-originated UI host, that already falls under the existing POST allowance for configured origins; no additional change. Confirmed during tasks.

---

## Summary of Resolved Unknowns

| Unknown | Resolution |
|---------|-----------|
| Compatible crate versions | `utoipa = "5"`, `utoipa-swagger-ui = "8"` (features = ["axum"]) — axum 0.7 compatible |
| UI/spec mounting | `SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi())` merged into router |
| Bearer scheme | `Modify` modifier adds HTTP Bearer (`bearer_jwt`, format JWT); referenced on protected paths |
| Logout 401 shape | Normalize to JSON `ErrorResponse` (minor, intentional; flagged for sign-off) |
| Response typing | Typed DTOs in `responses.rs`, names/values preserved |
| Consistency/coverage | Two `cargo test` integration tests (coverage guard + contract consistency) |
| Operational endpoints | Document `/api/health`; exclude `/metrics` |
| CORS/exposure | Reuse existing CorsLayer; verify GET reachability; no broadening |

All NEEDS CLARIFICATION items are resolved. Ready for Phase 1.
