# Quickstart & Validation: OpenAPI & Swagger Documentation

**Feature**: `specs/003-openapi-docs` | **Date**: 2026-07-15

This guide validates the feature end-to-end. It references the DTOs and guarantees in
[`data-model.md`](./data-model.md) and [`contracts/openapi-doc.md`](./contracts/openapi-doc.md)
rather than duplicating them.

## Prerequisites

- Rust toolchain (Edition 2024), `cargo`.
- PostgreSQL reachable via `DATABASE_URL` (see `.env.example`). Redis optional (see rate-limiter notes).
- A populated `.env` (copy from `.env.example`).

## Accessing the Documentation

Once the service is running:

| Resource | URL |
|----------|-----|
| Swagger UI | `http://localhost:3000/swagger-ui` |
| OpenAPI JSON | `http://localhost:3000/api-docs/openapi.json` |

> The default port is `3000` (configurable via `SERVER_PORT` in `.env`).

## 1. Build & static checks

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build
```

Expected: builds with **zero warnings** (constitution Principle III).

## 2. Run the service

```powershell
cargo run
```

Expected log line: `Listening` with the configured address.

## 3. Fetch the machine-readable spec

```powershell
curl http://localhost:3000/api-docs/openapi.json -o openapi.json
```

Expected: HTTP 200, `application/json`. The document satisfies the content guarantees in
`contracts/openapi-doc.md` (10 documented paths, `bearer_jwt` scheme, `/metrics` absent).

Validate it (any standard OpenAPI validator), e.g.:

```powershell
npx @redocly/cli lint openapi.json
```

Expected: zero structural errors (SC-003).

## 4. Open the interactive UI

Navigate a browser to:

```
http://localhost:3000/swagger-ui
```

Expected:
- All 10 documented endpoints listed with method, path, request/response schemas, and status codes.
- `POST /api/auth/logout` shows a required-auth (padlock) indicator; an **Authorize** button accepts a Bearer token.

## 5. Try a live request from the UI (SC-005)

1. Expand `POST /api/auth/login/password`.
2. "Try it out" → submit the seeded default user credentials (see `.env`).
3. Expected: HTTP 200 with an `AuthTokensResponse` body (`status: "authenticated"`, `user_id`, `access_token`, `refresh_token`).
4. Copy `access_token`, click **Authorize**, paste as the Bearer token.
5. Expand `POST /api/auth/logout`, provide a valid `session_id`, execute.
6. Expected: HTTP 200 with `StatusResponse` (`status: "logged_out"`).

Target: a new consumer completes the above in under 5 minutes without reading source code (SC-005).

## 6. Verify documented error outcomes are reproducible (SC-004)

From the UI or curl:
- Empty username/password → **422** `ErrorResponse`.
- Wrong credentials → **401** `ErrorResponse`.
- Logout without a Bearer token → **401** `ErrorResponse` (normalized JSON, per research Decision 4).
- Exceed the login rate limit (repeat rapidly) → **429** `ErrorResponse`.

Each observed status code and shape must match what the spec documents.

## 7. Run the automated guarantees

```powershell
cargo test
```

Expected: the following pass —
- `tests/openapi_spec_test.rs` — spec served, valid, security scheme present.
- `tests/openapi_coverage_test.rs` — every documented public endpoint present; `/metrics` absent (SC-008).
- `tests/docs_contract_consistency_test.rs` — generated spec matches `specs/*/contracts/*.md` on endpoints/methods/status codes (SC-007).

## 8. Confirm complementary Markdown contracts are intact (FR-010)

```powershell
Get-ChildItem specs/002-audit-security-hardening/contracts
```

Expected: `login-password.md`, `login-google.md`, `logout.md`, `refresh.md` still present.

## Acceptance mapping

| Step | Validates |
|------|-----------|
| 3, 7 | SC-001, SC-003, FR-001, FR-003–FR-006 |
| 4, 5 | US1, SC-002, SC-005, FR-002, FR-007 |
| 6 | SC-004, FR-008 |
| 7 | US2, US3, SC-006, SC-007, SC-008, FR-011, FR-012 |
| 8 | FR-010 |
| 1 | Constitution III (zero warnings, fmt, clippy) |
