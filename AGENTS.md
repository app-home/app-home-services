<!-- SPECKIT START -->
The current implementation plan is at `specs/003-openapi-docs/plan.md`.
Read it for context about technologies, project structure, and the
implementation approach for the OpenAPI & Swagger documentation feature.
<!-- SPECKIT END -->

## Project State (auto-updated by opencode)

### Architecture
Modular monolith with DDD + hexagonal architecture. Bounded contexts: `crates/auth/`, `crates/profiles/`, `crates/admin/`, `crates/shared/`, `crates/infrastructure/`. Main crate `src/` acts as composition root. Each context owns its domain, application ports/use-cases, adapters, and config. Infrastructure crate (`crates/infrastructure/`) provides db pool, telemetry/logging/metrics, rate limiter setup — shared by all bounded contexts. `src/api_doc.rs` contains the combined OpenAPI spec across all bounded contexts.

### Completed
- **DDD**: `UserAggregate` with domain events, aggregate methods (`add_session`, `invalidate_session`, `rotate_session`, etc.), `User::new()` with `validate_invariants()`.
- **Use cases**: `login_with_password`, `login_with_google`, `logout`, `refresh_token` — each returns `Vec<Event>`, published via `EventBus`.
- **Rate limiters moved into `crates/auth/`**: `memory_rate_limiter.rs`, `redis_rate_limiter.rs` under `auth::adapters::`.
- **All outbound adapters in `crates/auth/src/adapters/`**: postgres repos, JWT, GoogleAuth, audit, rate limiters.
- **Settings modular**: `auth::config::auth_settings::AuthSettings` + `shared::config::settings::Settings` (infra-only) + `infrastructure::config::Settings` (re-export from shared).
- **Infrastructure crate extracted**: `crates/infrastructure/` with `database::create_pool`, `telemetry::logging::init_logging`, `telemetry::metrics::install_prometheus_recorder`, `rate_limiter_setup::build_rate_limiters` + tests. `run_migrations` kept in `src/main.rs` due to `sqlx::migrate!` path resolution.
- **Inbound HTTP handlers moved into `crates/auth/`**: `login_routes.rs`, `logout_routes.rs`, `refresh_routes.rs`, `oauth_callback.rs`, `health_routes.rs`, `auth_middleware.rs`, `responses.rs`, `api_doc.rs` — all under `auth::adapters::inbound::`. `AppState` now lives in `auth::state::AppState` (no longer depends on `Settings`; takes `trusted_proxy_ips: Vec<IpAddr>` directly). `src/lib.rs` is a thin re-export layer.
- **All tests pass** (111 non-ignored + 7 admin crate tests + 34 ignored integration), 0 clippy warnings, fmt clean.
- **New bounded context: `crates/profiles/`**: User profiles context with `user_profiles` table, `ProfileRepository` port, Postgres implementation, value objects (`AvatarUrl`, `Bio`), use cases (`get_profile`, `update_profile`). HTTP handlers with JWT extraction (no base64 dep). Combined OpenAPI spec in `src/api_doc.rs` (replaces `auth::api_doc::ApiDoc`). Contracts at `specs/005-user-profiles/contracts/`.
- **New bounded context: `crates/admin/`**: Admin user management context. Extends `users` table with `role` column (migration 007). `Role` value object (user/admin), `AdminUser` entity, `AdminRepository` port, Postgres implementation, use cases (`list_users`, `get_user`, `update_user_role`). Admin guard checks JWT + DB role. Contracts at `specs/006-admin/contracts/`. Routes: `GET /api/admin/users`, `GET /api/admin/users/{id}`, `PUT /api/admin/users/{id}/role`.
- **Dependency graph**: `shared → auth → infrastructure → main → profiles → admin` (profiles and admin depend only on shared; no dep on auth).

### Active
- (none)

### Blocked
- (none)

### Next
Add admin unit tests (domain, use-cases with mock repo), or add more admin features (toggle user status, update role), or start another bounded context.
