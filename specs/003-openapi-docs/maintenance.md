# Maintenance: OpenAPI & Swagger Documentation

**Feature**: `specs/003-openapi-docs` | **Last updated**: 2026-07-15

This document describes how to maintain the OpenAPI specification and interactive documentation over time. It covers adding new endpoints, changing existing ones, updating the Markdown contracts, and running the automated consistency checks.

## How The Spec Is Generated

The OpenAPI document at `/api-docs/openapi.json` is generated at compile time by `utoipa` from:

1. **`#[utoipa::path]` annotations** on each handler function that define the HTTP method, path, request body, responses, and security scheme.
2. **`#[derive(ToSchema)]`** on request and response DTOs that define the typed schemas.
3. **`ApiDoc`** (`src/adapters/inbound/api_doc.rs`) that aggregates all annotated paths, schemas, and the Bearer JWT security scheme into a single `#[derive(OpenApi)]` struct.

The `SwaggerUi` mount in `main.rs` serves both the JSON document and the interactive UI from the same `ApiDoc::openapi()` value.

## Adding A New Public Endpoint

1. **Add handler + DTOs**: Create the handler in `src/adapters/inbound/`. Add `#[derive(Serialize, Deserialize, ToSchema)]` to any new request/response structs.
2. **Add `#[utoipa::path]`**: Annotate the handler with all documented status codes, request body, and security scheme where applicable. Use non-sensitive `#[schema(example = ...)]` values.
3. **Register in `ApiDoc`**: Add the handler's `__path_*` import and function reference to `paths(...)` in `ApiDoc`. Add any new schema types to `components(schemas(...))`.
4. **Register the route**: Add the route in `main.rs` with `.route(...)`.
5. **Update contracts**: Add or update the corresponding Markdown contract in `specs/*/contracts/`.
6. **Coverage test**: Update the `DOCUMENTED_PATH_METHODS` constant in `tests/openapi_coverage.rs`.
7. **Run validation**: `cargo test` — the coverage and consistency tests will verify the new endpoint is documented and matches its contracts.

## Changing An Existing Endpoint

1. **Change implementation**: Update the handler, request DTO, or response DTO.
2. **Update `#[utoipa::path]`**: If status codes, request/response shapes, or the security scheme changed, update the annotation.
3. **Update contracts**: Edit the corresponding Markdown contract to reflect changed request/response shapes or status codes.
4. **Run validation**: `cargo test` — the consistency test will flag any divergence between the spec and contracts.

## Removing An Endpoint

1. Remove the route from `main.rs`.
2. Remove the handler and its `#[utoipa::path]` annotation.
3. Remove the `__path_*` import and function reference from `ApiDoc::paths(...)`.
4. Remove any DTOs that are no longer used from `ApiDoc::components(schemas(...))`.
5. Update `DOCUMENTED_PATH_METHODS` in `tests/openapi_coverage.rs`.
6. Update or remove the corresponding Markdown contract.

## Operational Endpoint Policy (FR-015)

| Endpoint | Documented? | Reason |
|----------|-------------|--------|
| `/api/health` | ✅ Yes | JSON endpoint, useful to consumers |
| `/metrics` | ❌ No | Prometheus text format, in-cluster scraping only |

To change this policy, update the table above and add/remove the endpoint from `DOCUMENTED_PATH_METHODS` in `tests/openapi_coverage.rs`.

## Running The Checks

The following checks run automatically via `cargo test`:

| Test file | What it validates |
|-----------|-------------------|
| `tests/openapi_validity.rs` | Spec is structurally valid, has all DTOs, has security scheme |
| `tests/openapi_coverage.rs` | Every documented endpoint is present in the spec; `/metrics` is absent |
| `tests/responses_serde.rs` | All response DTOs serialize and deserialize correctly |
| `tests/markdown_contract_consistency.rs` | Generated spec matches `specs/*/contracts/*.md` on endpoints, methods, and status codes |

To run manually:

```bash
cargo test --test openapi_validity
cargo test --test openapi_coverage
cargo test --test markdown_contract_consistency
cargo test --test responses_serde
```

For external validation:

```bash
cargo run &
curl http://localhost:3000/api-docs/openapi.json -o openapi.json
npx @redocly/cli lint openapi.json
```

## Secrets Policy (FR-014)

All `#[schema(example = ...)]` values MUST be obvious placeholders:
- Username: `"jdoe"` (not a real username)
- Password: `"hunter2"` (not a real password)
- Token: `"<jwt>"` or `"eyJ...placeholder..."` (not a valid JWT)
- Session ID: a valid UUID format but not tied to any real session

## Adding a New Schema Type

1. Define the struct with `#[derive(Serialize, Deserialize, ToSchema)]` in `src/adapters/inbound/responses.rs` (or alongside the handler).
2. Add it to `components(schemas(...))` in `ApiDoc`.
3. Add non-sensitive `#[schema(example = ...)]` values to each field.

## Useful Links

- **utoipa docs**: https://docs.rs/utoipa/latest/utoipa/
- **utoipa-swagger-ui docs**: https://docs.rs/utoipa-swagger-ui/latest/utoipa_swagger_ui/
- **OpenAPI 3.x spec**: https://spec.openapis.org/oas/v3.0.3
