<!-- SPECKIT START -->
The active work is issue #194: bringing the spec plan docs back in sync with
the current code layout after the modular monolith crate split (#191). Several
`specs/*/plan.md` files still referenced the old flat `src/` tree; they are
being updated to the workspace-crate paths (`crates/auth/`, `crates/shared/`,
`crates/infrastructure/`) and the composition-root files (`src/api_doc.rs`,
`src/health.rs`, `src/router.rs`, `src/security_headers.rs`) they actually
live in. See the "Active" section below for the current step.
<!-- SPECKIT END -->

## Project State (auto-updated by opencode)

### Architecture
Modular monolith with DDD + hexagonal architecture. Bounded contexts: `crates/auth/`, `crates/profiles/`, `crates/admin/`, `crates/shared/`, `crates/infrastructure/`. Main crate `src/` acts as composition root. Each context owns its domain, application ports/use-cases, adapters, and config. Infrastructure crate (`crates/infrastructure/`) provides db pool, telemetry/logging/metrics, rate limiter setup — shared by all bounded contexts. `src/api_doc.rs` contains the combined OpenAPI spec across all bounded contexts.

### Completed
- **DDD**: `UserAggregate` with domain events, aggregate methods (`add_session`, `invalidate_session`, `rotate_session`, etc.), `User::new()` with `validate_invariants()`.
- **Use cases**: `login_with_password`, `login_with_google`, `logout`, `refresh_token` — each returns `Vec<Event>`, published via `EventBus`.
- **Rate limiters in `crates/infrastructure/`**: `RateLimiter` trait in `shared::ports`, `memory.rs`/`redis.rs` implementations + `build_rate_limiters` under `crates/infrastructure/`.
- **Outbound adapters in `crates/auth/src/adapters/`**: postgres repos, JWT (`jwt_service.rs`), GoogleAuth, audit event handler.
- **Settings modular**: `auth::config::auth_settings::AuthSettings` + `shared::config::settings::Settings` (infra-only) + `infrastructure::config::Settings` (re-export from shared).
- **Infrastructure crate extracted**: `crates/infrastructure/` with `database::create_pool`, `telemetry::logging::init_logging`, `telemetry::metrics::install_prometheus_recorder`, `rate_limiter_setup::build_rate_limiters` + tests. `run_migrations` kept in `src/main.rs` due to `sqlx::migrate!` path resolution.
- **Inbound HTTP handlers moved into `crates/auth/`**: `login_routes.rs`, `logout_routes.rs`, `refresh_routes.rs`, `oauth_callback.rs`, `responses.rs` under `crates/auth/src/adapters/inbound/`; JWT extraction middleware (`AuthenticatedUser`/`JwtVerification`) in `crates/shared/src/auth.rs`; `health_check` in `src/health.rs`; combined OpenAPI spec in `src/api_doc.rs`. `AppState` now lives in `auth::state::AppState` (no longer depends on `Settings`; takes `trusted_proxy_ips: Vec<IpAddr>` directly). `src/lib.rs` is a thin re-export layer.
- **All tests pass** (111 non-ignored + 7 admin crate tests + 49 ignored integration), 0 clippy warnings, fmt clean.
- **HTTP security headers (#90)**: HSTS, X-Content-Type-Options, X-Frame-Options, Referrer-Policy applied to every response via `tower-http` (`SetResponseHeaderLayer::overriding`). CSP deliberately omitted (JSON API; Swagger UI incompatible). In-process test `security_headers_are_applied_to_every_response_including_404s` in `tests/router_test.rs` validates all four.
- **CI dependency security (#91)**: `cargo-audit` + `cargo-deny` jobs in CI; `deny.toml` with license allow-list, private workspace crates, `publish=false` on all 6 packages.
- **New bounded context: `crates/profiles/`**: User profiles context with `user_profiles` table, `ProfileRepository` port, Postgres implementation, value objects (`AvatarUrl`, `Bio`), use cases (`get_profile`, `update_profile`). HTTP handlers with JWT extraction (no base64 dep). Combined OpenAPI spec in `src/api_doc.rs` (replaces `auth::api_doc::ApiDoc`). Contracts at `specs/005-user-profiles/contracts/`.
- **New bounded context: `crates/admin/`**: Admin user management context. Extends `users` table with `role` column (migration 007). `Role` value object (user/admin), `AdminUser` entity, `AdminRepository` port, Postgres implementation, use cases (`list_users`, `get_user`, `update_user_role`). Admin guard checks JWT + DB role. Contracts at `specs/006-admin/contracts/`. Routes: `GET /api/admin/users`, `GET /api/admin/users/{id}`, `PUT /api/admin/users/{id}/role`. Admin self-demotion blocked (#92): `update_user_role` takes `actor_id` and returns `CannotChangeOwnRole` (403) when `actor_id == user_id`.
- **Dependency graph**: `shared → auth → infrastructure → main → profiles → admin` (profiles and admin depend only on shared; no dep on auth).
- **Bcrypt cost 12 (OWASP) (#94)**: centralized `DEFAULT_BCRYPT_COST = 12` + fail-fast `BCRYPT_COST` override (`validate_bcrypt_cost`, `12..=31`); timing-safe not-found path uses a per-cost precomputed dummy hash cache.
- **Bounded bcrypt concurrency (#175)**: `BcryptLimiter` (`bcrypt_task.rs`) routes every hash/verify through `spawn_blocking` with a `BCRYPT_MAX_CONCURRENT` semaphore (max 512, Tokio's blocking-pool ceiling).
- **Composition root split (#191)**: `build_router` (in `src/router.rs`) and `build_cors_layer` (in `src/cors.rs`) extracted out of `main` so the app is constructible/testable in-process; background pollers/flusher moved to `infrastructure::telemetry::pollers`.
- **Spec plans synced with code layout (#194)**: `specs/{001,002,003,004,008,010}/plan.md` (+ 003's data-model/research/maintenance/quickstart) updated from the pre-split flat `src/` tree to the current `crates/` + composition-root layout; stale test names corrected (`openapi_spec_served`, `openapi_coverage`, `markdown_contract_consistency`); `tests/integration/security_headers_test.rs` → in-process `tests/router_test.rs` assertions.

### Active
- `chore/194-fix-spec-sync`: spec-plan sync with current code layout; PR open against `development` will close #194.

### Blocked
- (none)

### Next
Merge the #194 spec-sync PR, then the `development` → `main` promotion (#193 carries #191; a second promotion carries #194). Afterward: add admin unit tests, or start another bounded context.
