---

description: "Task list for user authentication feature implementation"

---

# Tasks: User Authentication

**Input**: Design documents from `specs/001-user-auth/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Test tasks are included per the Test-First and Quality Gates constitution principle.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/`, `tests/` at repository root
- Paths below follow the Hexagonal Architecture structure defined in plan.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [x] T001 Create Rust project with `cargo init --lib` and configure `Cargo.toml` with dependencies (axum, tokio, sqlx, bcrypt, uuid, tracing, serde, reqwest, thiserror)
- [x] T002 [P] Configure `rustfmt.toml` and `.clippy.toml` for formatting and linting
- [x] T003 Create directory structure per plan.md: `src/domain/entities/`, `src/application/use_cases/`, `src/application/ports/`, `src/adapters/inbound/`, `src/adapters/outbound/`, `src/infrastructure/database/migrations/`, `src/infrastructure/config/`, `src/infrastructure/telemetry/`
- [x] T004 Create `.env.example` with all required environment variables (DATABASE_URL, DEFAULT_USER_USERNAME, DEFAULT_USER_PASSWORD, DEFAULT_USER_EMAIL, GOOGLE_CLIENT_ID, SERVER_HOST, SERVER_PORT)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T005 [P] Implement configuration module in `src/infrastructure/config/settings.rs` — load env vars with `dotenvy`, struct for Settings with database, auth, and server config
- [x] T006 [P] Implement telemetry module in `src/infrastructure/telemetry/logging.rs` — initialize `tracing-subscriber` with structured logging
- [x] T007 Create SQLx migration `migrations/001_create_users_table.sql` per data-model.md (UUID PK, username, email, display_name, password_hash, auth_provider, timestamps)
- [x] T008 Create SQLx migration `migrations/002_create_user_actions_table.sql` per data-model.md (UUID PK, user_id FK to users, auth_method, created_at)
- [x] T009 Create SQLx migration `migrations/003_seed_default_user.sql` — insert default user with configurable bcrypt hash
- [x] T010 Implement database connection pool setup in `src/infrastructure/database/db.rs` — `PgPool` creation from `DATABASE_URL`
- [x] T011 Implement application state in `src/main.rs` — shared state with db pool and settings; Axum router skeleton with health check endpoint at `GET /api/health`
- [x] T012 [P] Implement domain error types in `src/domain/errors.rs` — `AuthError` enum with variants (InvalidCredentials, UserNotFound, TokenVerificationFailed, InternalError); derive thiserror::Error

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Username/Password Login (Priority: P1) 🎯 MVP

**Goal**: Authenticate the pre-seeded default user via username and password, record successful login in user_actions, reject invalid attempts with generic errors

**Independent Test**: Can be tested by sending a valid username/password to `POST /api/auth/login/password` and verifying a 200 response with `user_id`; an invalid password returns 401 with `"Invalid username or password"`; a non-existent username returns the same 401 message; query user_actions table to confirm audit entry

### Tests for User Story 1 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T013 [P] [US1] Unit test for password hashing and verification in `tests/unit/password_test.rs`
- [x] T014 [P] [US1] Unit test for domain `User` entity validation in `tests/unit/user_test.rs`
- [x] T015 [US1] Integration test for password login endpoint in `tests/integration/login_password_test.rs` — test valid login, wrong password, non-existent username, and audit entry creation

### Implementation for User Story 1

- [x] T016 [P] [US1] Create `User` domain entity in `src/domain/entities/user.rs` — struct with id, username, email, display_name, password_hash, auth_provider, created_at, updated_at; implement `verify_password(password: &str) -> bool`
- [x] T017 [P] [US1] Create `UserAction` domain entity in `src/domain/entities/user_action.rs` — struct with id, user_id, auth_method, created_at
- [x] T018 [US1] Implement `UserRepository` port trait in `src/application/ports/user_repository.rs` — `find_by_username(username: &str) -> Result<Option<User>>`, `find_by_email(email: &str) -> Result<Option<User>>`, `create(user: NewUser) -> Result<User>`
- [x] T019 [US1] Implement `LoginWithPassword` use case in `src/application/use_cases/login_with_password.rs` — validate credentials against stored hash, return domain User on success, return InvalidCredentials on failure (generic error, doesn't distinguish user-not-found from wrong-password)
- [x] T020 [US1] Implement `PostgresUserRepo` in `src/adapters/outbound/postgres_user_repo.rs` — SQLx query implementation of UserRepository port; `find_by_username` with `SELECT * FROM users WHERE username = $1`
- [x] T021 [US1] Implement password login HTTP handler in `src/adapters/inbound/login_routes.rs` — `POST /api/auth/login/password` accepting JSON `{ username, password }`, calling `LoginWithPassword` use case
- [x] T022 [US1] Implement `RecordAuditEntry` use case in `src/application/use_cases/record_audit_entry.rs` — insert into user_actions table with user_id, auth_method ("password" or "google_oauth"), and timestamp
- [x] T023 [US1] Wire routes in `src/main.rs` — add login routes to Axum router; pass shared state (db pool, settings) to handlers
- [x] T024 [US1] Add validation and error handling — 422 for missing fields, 401 for invalid credentials, 500 for internal errors; log all attempts via tracing

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently — default user can log in with password, invalid attempts return generic errors, audit entries are recorded

---

## Phase 4: User Story 2 - Google OAuth Login (Priority: P2)

**Goal**: Authenticate users via Google OAuth (Sign in with Google), auto-create new users on first login, record all successful logins in user_actions

**Independent Test**: Can be tested by sending a valid Google ID token to `POST /api/auth/login/google` and verifying a 200 response with `user_id` and `is_new_user` flag; an invalid token returns 401 with `"Authentication failed"`; first-time login with a new Google email creates a user record; subsequent logins with the same token return `is_new_user: false`

### Tests for User Story 2 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T025 [P] [US2] Unit test for Google ID token verification logic in `tests/unit/google_token_test.rs` — mock Google JWKS response, test valid/invalid/expired tokens
- [x] T026 [US2] Integration test for Google login endpoint in `tests/integration/login_google_test.rs` — test first-time login (auto-registration), returning user login, invalid token rejection, and audit entry creation

### Implementation for User Story 2

- [x] T027 [P] [US2] Implement `AuthProvider` port trait in `src/application/ports/auth_provider.rs` — `verify_id_token(token: &str) -> Result<GoogleUserInfo>` where `GoogleUserInfo` contains email, name
- [x] T028 [US2] Implement `GoogleAuthProvider` in `src/adapters/outbound/google_auth_provider.rs` — fetch Google JWKS keys, verify JWT signature/issuer/audience, extract email and name from payload
- [x] T029 [US2] Implement `LoginWithGoogle` use case in `src/application/use_cases/login_with_google.rs` — verify token via `AuthProvider`, find existing user by email, create if not found, return `(User, is_new_user)` tuple
- [x] T030 [US2] Add `create` method to `PostgresUserRepo` in `src/adapters/outbound/postgres_user_repo.rs` — `INSERT INTO users (id, username, email, display_name, password_hash, auth_provider) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *`
- [x] T031 [US2] Add `find_by_email` method to `PostgresUserRepo` in `src/adapters/outbound/postgres_user_repo.rs` — `SELECT * FROM users WHERE email = $1`
- [x] T032 [US2] Implement Google login HTTP handler in `src/adapters/inbound/oauth_callback.rs` — `POST /api/auth/login/google` accepting JSON `{ id_token }`, calling `LoginWithGoogle` use case, returning `{ status, user_id, is_new_user }` or error
- [x] T033 [US2] Wire Google login route in `src/main.rs` — add to Axum router
- [x] T034 [US2] Add validation and error handling — 422 for missing id_token, 401 for invalid token, 500 for internal errors; log all attempts via tracing

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently — both login methods work, Google auto-registration creates users, all successful logins are audited

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect both user stories

- [x] T035 [P] Add request logging middleware in `src/main.rs` — tracing span for each request with method, path, status, duration
- [x] T036 [P] Add rate limiting to login endpoints — reasonable per-IP limit to prevent brute-force attacks
- [x] T037 Add `.env` to `.gitignore` and ensure `Cargo.toml` has all required dependencies (bcrypt, uuid with v7, sqlx with postgres+uuid+runtime-tokio, axum, tokio, serde, reqwest, tracing, tracing-subscriber, dotenvy, thiserror)
- [x] T038 Add unit tests for `RecordAuditEntry` use case in `tests/unit/audit_test.rs`
- [x] T039 Add database integration test for migrations in `tests/integration/database_test.rs` — verify both tables exist and seed user is present
- [x] T040 Run `cargo fmt`, `cargo clippy` and fix all warnings
- [x] T041 Run full `cargo test` suite — all tests pass, update quickstart.md if needed
- [x] T042 Run quickstart.md validation scenarios manually and document results

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion — BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User stories can then proceed in parallel (if staffed)
  - Or sequentially in priority order (P1 → P2)
- **Polish (Final Phase)**: Depends on both user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) — No dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) — Uses the same database pool, UserRepository, and UserAction repository as US1 but is independently testable

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Domain entities before services
- Port interfaces before adapters
- Use cases before HTTP handlers
- Core implementation before integration
- Story complete before moving to next priority

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel
- All Foundational tasks marked [P] can run in parallel
- Once Foundational phase completes, User Story 1 and User Story 2 can start in parallel
- All tests for a user story marked [P] can run in parallel
- Models/entities within a story marked [P] can run in parallel
- Different user stories can be worked on in parallel by different team members

---

## Parallel Example: User Story 1

```bash
# Launch all tests for User Story 1 together:
Task: T013 "Unit test for password hashing in tests/unit/password_test.rs"
Task: T014 "Unit test for User entity validation in tests/unit/user_test.rs"

# Launch all entities for User Story 1 together:
Task: T016 "Create User domain entity in src/domain/entities/user.rs"
Task: T017 "Create UserAction domain entity in src/domain/entities/user_action.rs"
```

## Parallel Example: User Story 2

```bash
# Launch all tests for User Story 2 together:
Task: T025 "Unit test for Google ID token verification in tests/unit/google_token_test.rs"

# Launch port and provider together:
Task: T027 "Implement AuthProvider port trait in src/application/ports/auth_provider.rs"
Task: T029 "Implement LoginWithGoogle use case in src/application/use_cases/login_with_google.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks all stories)
3. Complete Phase 3: User Story 1 (Username/Password Login)
4. **STOP and VALIDATE**: Test User Story 1 independently via curl commands from quickstart.md
5. Deploy/demo if ready — password login with audit logging works

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Deploy/Demo (MVP!)
3. Add User Story 2 → Test independently → Deploy/Demo
4. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1
   - Developer B: User Story 2
3. Stories complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- Verify tests fail before implementing
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
