# Tasks: Audit & Security Hardening

**Input**: Design documents from `specs/002-audit-security-hardening/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included as requested by security hardening requirements

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US3, US4, US5)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/`, `tests/`, `migrations/` at repository root

---

## Phase 1: Setup — Environment & Configuration

**Purpose**: Extend environment configuration with new settings for the security hardening feature

- [x] T001 Add new env vars to `Settings` in `src/infrastructure/config/settings.rs`: `jwt_secret`, `rate_limit_max_attempts`, `rate_limit_window_seconds`, `access_token_expiry_minutes`, `refresh_token_expiry_days`, `cors_allowed_origins`
- [x] T002 [P] Update `.env.example` with new env vars and documentation
- [x] T003 [P] Update `AGENTS.md` to reference `specs/002-audit-security-hardening/plan.md`

---

## Phase 2: Foundational — Domain Entities, Ports & Migrations (BLOCKING all stories)

**Purpose**: Core infrastructure that MUST be complete before any user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T004 Create migration `migrations/003_create_sessions_table.sql` with UUID PK, user_id FK, refresh_token_hash VARCHAR(255), expires_at TIMESTAMPTZ, is_active BOOLEAN, created_at TIMESTAMPTZ
- [x] T005 Create migration `migrations/004_extend_user_actions.sql`: ALTER user_actions ADD COLUMN session_id UUID REFERENCES sessions(id), ADD COLUMN event_type VARCHAR(20) NOT NULL DEFAULT 'login'; CREATE INDEXes
- [x] T006 [P] Create `Session` domain entity in `src/domain/entities/session.rs` with fields: id, user_id, refresh_token_hash, expires_at, is_active, created_at
- [x] T007 [P] Create `NewSession` struct in `src/domain/entities/session.rs`
- [x] T008 [P] Extend `UserAction` domain entity in `src/domain/entities/user_action.rs`: add `event_type: String` and `session_id: Option<Uuid>` fields
- [x] T009 [P] Extend `NewUserAction` in `src/domain/entities/user_action.rs`: add `event_type: String` and `session_id: Option<Uuid>` fields
- [x] T010 [P] Add `AuthError` variants in `src/domain/errors.rs`: `SessionExpired`, `SessionInvalidated`, `RateLimited`, `StartupError(String)`
- [x] T011 [P] Create `SessionRepository` port in `src/application/ports/session_repository.rs` with methods: `create(session) -> Result<Session>`, `find_by_refresh_token_hash(hash) -> Result<Option<Session>>`, `invalidate(session_id) -> Result<()>`, `invalidate_previous_for_user(user_id, exclude_session_id) -> Result<()>`
- [x] T012 [P] Create `RateLimiter` port in `src/application/ports/rate_limiter.rs` with method: `check_rate_limit(key: &str) -> Result<()>`
- [x] T013 [P] Create `JwtService` port in `src/application/ports/jwt_service.rs` with methods: `create_access_token(user_id, session_id) -> Result<String>`, `create_refresh_token(user_id, session_id) -> Result<String>`, `validate_access_token(token) -> Result<Claims>`, `validate_refresh_token(token) -> Result<Claims>`
- [x] T014 [P] Create `Claims` struct in `src/application/ports/jwt_service.rs` with fields: sub (user_id), sid (session_id), exp, iat
- [x] T015 [P] Update `UserRepository::create_user_action` signature in `src/application/ports/user_repository.rs` to accept `event_type: &str` and `session_id: Option<Uuid>` parameters
- [x] T016 [P] Add new modules to `src/domain/entities/mod.rs`
- [x] T017 [P] Add new modules to `src/application/ports/mod.rs`

**Checkpoint**: Foundation ready — all domain entities, ports, and migrations exist

---

## Phase 3: User Story 3 — Resilient Startup (Priority: P1) 🎯 MVP

**Goal**: System aborts startup with clear error on DB failure during default user check; logs all 3 outcomes without exposing password

**Independent Test**: Fresh DB → "Default user created successfully" log; Restart → "Default user already exists" log; Unreachable DB → process exits with error

### Implementation for User Story 3

- [x] T018 [US3] Refactor `seed_default_user` in `src/main.rs` to return `Result<bool>`: Ok(true) = already exists, Ok(false) = created, Err = abort startup with `process::exit(1)`
- [x] T019 [US3] Add structured tracing for 3 outcomes: info "Default user already exists", info "Default user created successfully", error "Default user check failed: {reason}"
- [x] T020 [US3] Verify no password is logged: use structured fields `tracing::info!(username = %settings.default_user_username, "message")` instead of string interpolation

**Checkpoint**: Startup hardening complete — system fails safely on DB errors

---

## Phase 4: User Story 1 + User Story 2 — Session-based Auth & Extended Audit Trail (Priority: P1)

**Goal**: Users receive JWT tokens on login, can explicitly log out to invalidate sessions, and the audit trail records login/logout/refresh events linked to sessions

**Independent Test**: Login → receive access+refresh tokens → refresh → receive new pair → logout → verify old refresh fails + audit trail has login, refresh, logout entries with matching session_id

### Implementation for User Story 1 & 2

- [x] T021 [P] [US1] Implement `JwtService` adapter in `src/adapters/outbound/jwt_service.rs`: HMAC-SHA256 signing using `jsonwebtoken` crate, access_token=15min, refresh_token=7d (configurable via settings)
- [x] T022 [P] [US1] Implement `PostgresSessionRepo` adapter in `src/adapters/outbound/postgres_session_repo.rs`: implement `SessionRepository` trait with bcrypt-hashed refresh_token storage
- [x] T023 [US1] Update `record_audit_entry` use case in `src/application/use_cases/record_audit_entry.rs`: accept `event_type: &str` and `session_id: Option<Uuid>` parameters, pass to repo
- [x] T024 [US1] Update `login_with_password` use case in `src/application/use_cases/login_with_password.rs`: return a session token pair on success; accept JwtService and SessionRepository via params
- [x] T025 [US1] Update `login_with_google` use case in `src/application/use_cases/login_with_google.rs`: return a session token pair on success; accept JwtService and SessionRepository via params
- [x] T026 [US1] Create `logout` use case in `src/application/use_cases/logout.rs`: validate access token claims, invalidate session via SessionRepository, record "logout" audit event with session_id
- [x] T027 [US1] Create `refresh_token` use case in `src/application/use_cases/refresh_token.rs`: validate old refresh token, verify session is active via SessionRepository, invalidate old session, create new session, return new token pair, record "refresh" audit event
- [x] T028 [P] Update `src/application/use_cases/mod.rs` with new modules and updated function signatures
- [x] T029 [US1] Update `PostgresUserRepo::create_user_action` in `src/adapters/outbound/postgres_user_repo.rs`: update `UserActionRow` struct and SQL to insert `session_id` and `event_type`
- [x] T030 [P] Add new adapter modules to `src/adapters/outbound/mod.rs`
- [x] T031 [US1] Create auth middleware in `src/adapters/inbound/auth_middleware.rs`: extract `Authorization: Bearer <token>`, validate via `JwtService::validate_access_token`, inject user_id and session_id into request extensions
- [x] T032 [US1] Update `login_password_handler` in `src/adapters/inbound/login_routes.rs`: on success create session + tokens, return `{ access_token, refresh_token, user_id }`, record "login" audit event with session_id
- [x] T033 [US1] Update `login_google_handler` in `src/adapters/inbound/oauth_callback.rs`: on success create session + tokens, return `{ access_token, refresh_token, user_id, is_new_user }`, record "login" audit event with session_id
- [x] T034 [US1] Create `POST /api/auth/logout` handler in `src/adapters/inbound/logout_routes.rs`: use auth middleware, invoke logout use case, return `{ "status": "logged_out" }`
- [x] T035 [US1] Create `POST /api/auth/refresh` handler in `src/adapters/inbound/refresh_routes.rs`: accept `{ refresh_token }`, invoke refresh_token use case, return `{ access_token, refresh_token }`
- [x] T036 [P] Update `src/adapters/inbound/mod.rs` with new route modules
- [x] T037 [US1] Add `jwt_service`, `session_repo`, `rate_limiter: Arc<Mutex<MemoryRateLimiter>>` to `AppState` in `src/lib.rs` and update `AppState::new` constructor
- [x] T038 [US1] Wire new routes in `src/main.rs`: add `/api/auth/logout`, `/api/auth/refresh` routes; apply auth middleware to protected routes
- [x] T039 Fix `addr` in `src/main.rs` to use `settings.server_host` and `settings.server_port` instead of hardcoded `"0.0.0.0"` and `3000`
- [x] T040 [US2] Update `seed_default_user` in `src/main.rs` to also record a "login" audit event for the default user seed if the user was just created

**Checkpoint**: Full session lifecycle works — login returns tokens, refresh rotates tokens, logout invalidates session, audit trail has all events

---

## Phase 5: User Story 4 — Rate-limited Authentication (Priority: P2)

**Goal**: Failed login attempts are rate-limited per IP with uniform response timing

**Independent Test**: N consecutive failed logins from same IP → 429 error; response timing for valid vs invalid usernames is indistinguishable

### Implementation for User Story 4

- [ ] T041 [P] [US4] Implement `MemoryRateLimiter` adapter in `src/adapters/outbound/memory_rate_limiter.rs`: in-memory sliding window per IP key, configurable max_attempts and window_seconds
- [ ] T042 [US4] Integrate rate limiter into `login_password_handler` in `src/adapters/inbound/login_routes.rs`: check rate limit by client IP (`req.extensions()` or `req.headers()`) before credential validation, return 429 if exceeded, clear counter on success
- [ ] T043 [US4] Add uniform timing to failed login responses in `src/adapters/inbound/login_routes.rs`: add small fixed delay (e.g., 50ms) via `tokio::time::sleep` on all failed responses to mask timing differences

**Checkpoint**: Brute-force protection active — repeated failures trigger rate limit; no timing side-channels

---

## Phase 6: User Story 5 — Restricted CORS Policy (Priority: P2)

**Goal**: Cross-origin API access is restricted to configured origins; same-origin only by default

**Independent Test**: Request from allowed origin → Access-Control-Allow-Origin present; request from disallowed origin → no CORS headers

### Implementation for User Story 5

- [ ] T044 [US5] Promote `tower-http` from dev-dependencies to dependencies in `Cargo.toml` (add under `[dependencies]`, remove from `[dev-dependencies]`)
- [ ] T045 [US5] Add CORS middleware layer in `src/main.rs` using `tower_http::cors::CorsLayer`: parse `cors_allowed_origins` from settings, split by comma, configure `AllowOrigin::list`; if empty use `AllowOrigin::exact("")` to deny all (same-origin only)

**Checkpoint**: CORS explicitly restricted — no accidental open access

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [x] T046 [P] Add unit tests for Session entity in `tests/unit/session_test.rs`: creation, expiry validation, is_active transition
- [x] T047 [P] Add unit tests for MemoryRateLimiter in `tests/unit/rate_limiter_test.rs`: sliding window, threshold enforcement, reset after window
- [x] T048 [P] Add unit tests for JwtService in `tests/unit/jwt_test.rs`: token creation, validation, expiry, tamper detection
- [x] T049 [P] Add integration test for logout flow in `tests/integration/logout_test.rs`: login → logout → verify session invalidated + audit entry
- [x] T050 [P] Add integration test for token refresh + rotation in `tests/integration/refresh_test.rs`: login → refresh → old refresh rejected → new refresh works
- [x] T051 [P] Add integration test for rate limiting in `tests/integration/rate_limit_test.rs`: send N+1 failed attempts, verify 429, wait for window, verify reset
- [x] T052 [P] Add integration test for CORS restrictions in `tests/integration/cors_test.rs`: request from disallowed origin → no CORS headers; request from allowed origin → CORS headers present
- [x] T053 [P] Add integration test for startup hardening in `tests/integration/startup_test.rs`: verify process exits with error on unreachable DB
- [x] T054 Update existing integration tests in `tests/integration/login_password_test.rs` and `tests/integration/login_google_test.rs` to expect new token-based response format
- [x] T055 Run `cargo fmt && cargo clippy && cargo test` — zero warnings, all tests green
- [ ] T056 Commit with semantic message

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion — BLOCKS all user stories
- **US3 — Startup (Phase 3)**: Depends on Phase 1 + Phase 2 — can run independently in parallel with US1/US2
- **T040 → T018 dependency**: T040 modifies `seed_default_user` in `src/main.rs`; must run AFTER T018 which refactors the same function
- **US1+US2 — Session & Audit (Phase 4)**: Depends on Phase 1 + Phase 2
- **US4 — Rate Limiting (Phase 5)**: Depends on Phase 1 + Phase 2. Ideal order: after US1 (modifies same login handler)
- **US5 — CORS (Phase 6)**: Depends on Phase 1 only — can run in parallel with any story
- **Polish (Phase 7)**: Depends on all desired stories

```
Phase 1: Setup
    │
Phase 2: Foundational (BLOCKS all)
    │
    ├──> Phase 3: US3 (Startup) ─── independent parallel track
    │
    ├──> Phase 4: US1+US2 (Session & Audit) ─── main track
    │
    ├──> Phase 6: US5 (CORS) ─── independent parallel track
    │
    └──> Phase 5: US4 (Rate Limiting) ─── after US1 (same login handler)
                │
Phase 7: Polish ─── after all
```

### Parallel Opportunities

- All [P] tasks within a phase can run in parallel
- Phase 3 (US3) can run in parallel with Phase 4 (US1)
- Phase 6 (US5) can run in parallel with Phase 3 and Phase 4
- Phase 5 (US4) should run after Phase 4 US1 (modifies same files)

### Within Each User Story

- Entity/domain changes before ports
- Ports before adapters
- Adapters before wiring/routes
- Test before implementation (if TDD)

### Parallel Example: Phase 4 — US1+US2 Models & Adapters

```bash
# All adapter implementations can launch in parallel:
Task: "Implement JwtService adapter in src/adapters/outbound/jwt_service.rs"
Task: "Implement PostgresSessionRepo in src/adapters/outbound/postgres_session_repo.rs"

# All use cases can launch in parallel (after adapters):
Task: "Create logout use case in src/application/use_cases/logout.rs"
Task: "Create refresh_token use case in src/application/use_cases/refresh_token.rs"

# All handlers can launch in parallel (after use cases):
Task: "Create POST /api/auth/logout handler in src/adapters/inbound/logout_routes.rs"
Task: "Create POST /api/auth/refresh handler in src/adapters/inbound/refresh_routes.rs"
```

---

## Implementation Strategy

### MVP First (Phase 3 — US3: Startup Hardening)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational
3. Complete Phase 3: US3 (Startup Hardening)
4. **STOP and VALIDATE**: Test startup hardening independently
5. Deploy/demo if ready

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add US3 (Startup Hardening) → Test → Deploy/Demo (#1)
3. Add US1+US2 (Session & Audit) → Test → Deploy/Demo (#2)
4. Add US4 (Rate Limiting) → Test → Deploy/Demo (#3)
5. Add US5 (CORS) → Test → Deploy/Demo (#4)
6. Each increment adds value without breaking previous increments

### Parallel Team Strategy

With multiple developers:
1. Team completes Setup + Foundational together
2. Developer A: Phase 3 (US3) + Phase 6 (US5) — independent, minimal deps
3. Developer B: Phase 4 (US1+US2) — main feature
4. Developer C: Phase 5 (US4) — after Developer B finishes login handler changes
5. Team: Phase 7 (Polish) together

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
