# Implementation Plan: User Authentication

**Branch**: `N/A` | **Date**: 2026-07-01 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-user-auth/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

Add user authentication with two methods: username/password (single pre-seeded default user, no self-registration) and Google OAuth (auto-registers new Google users). Every successful login is recorded in a `user_actions` audit table. All tables use UUID primary keys. Passwords are stored hashed only.

## Technical Context

**Language/Version**: Rust (Edition 2024)

**Primary Dependencies**: Axum (HTTP), Tokio (async runtime), SQLx (PostgreSQL), `tracing` (logging), reqwest (Google token verification), `oauth2` or raw OpenID Connect verification

**Storage**: PostgreSQL (via SQLx migrations)

**Testing**: `cargo test` (unit tests for domain rules, integration tests for adapters, database tests for persistence)

**Target Platform**: Linux server (web service)

**Project Type**: web-service

**Performance Goals**: Login flow completes within 2 seconds for 95% of requests under normal load

**Constraints**: 
- UUID primary keys for all tables (per constitution v1.1.0)
- Passwords must be hashed using bcrypt/Argon2
- Hexagonal Architecture: domain must not depend on infrastructure
- No plain-text passwords in any log, response, or database

**Scale/Scope**: Small-scale internal service; single default local user; auto-registered Google users

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

**I. Hexagonal Architecture (NON-NEGOTIABLE)**
- Domain entities (User, UserAction) must be pure domain models with no infrastructure dependencies.
- Authentication ports/interfaces in application layer; adapters (HTTP inbound, PostgreSQL outbound, Google OAuth outbound) in infrastructure.
- PASS: Design follows Ports/Adapters pattern.

**II. Domain-Driven Design**
- User and UserAction domain entities with business rules (password hashing, auth method validation).
- Use cases: LoginWithPassword, LoginWithGoogle, RecordAuditEntry.
- PASS: Entities represent core business concepts.

**III. Rust-First Development**
- Rust Edition 2024, cargo fmt, cargo clippy, zero warnings.
- Result-based error handling with thiserror.
- PASS: Standard Rust project conventions.

**IV. Test-First and Quality Gates (NON-NEGOTIABLE)**
- Unit tests for domain password validation, auth method logic.
- Integration tests for HTTP login endpoints and DB persistence.
- Database tests for migrations and queries.
- PASS: All levels covered.

**V. PostgreSQL Persistence Standards**
- SQLx for database access, migrations for schema changes.
- UUID primary keys for users and user_actions tables.
- PASS: Explicitly required by spec and constitution.

**VI. Observability and Reliability**
- Structured logging with tracing for login attempts (success/failure).
- Health check endpoint for DB connectivity.
- PASS: Standard observability practices.

**GATE RESULT**: All gates pass. No complexity justification needed.

## Project Structure

### Documentation (this feature)

```text
specs/001-user-auth/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
crates/auth/src/
├── domain/
│   ├── entities/
│   │   ├── user.rs
│   │   ├── user_action.rs
│   │   └── session.rs
│   ├── services/
│   │   └── password_verification.rs
│   ├── aggregate.rs
│   └── errors.rs
│
├── application/
│   ├── use_cases/
│   │   ├── login_with_password.rs
│   │   ├── login_with_google.rs
│   │   ├── record_audit_entry.rs
│   │   ├── logout.rs
│   │   └── refresh_token.rs
│   └── ports/
│       ├── user_repository.rs
│       ├── auth_provider.rs
│       ├── session_repository.rs
│       └── jwt_service.rs
│
├── adapters/
│   ├── inbound/
│   │   ├── login_routes.rs
│   │   ├── oauth_callback.rs
│   │   ├── refresh_routes.rs
│   │   ├── logout_routes.rs
│   │   └── responses.rs
│   ├── postgres_user_repo.rs
│   ├── postgres_session_repo.rs
│   ├── jwt_service.rs
│   ├── google_auth_provider.rs
│   └── audit_event_handler.rs
│
├── config/
│   └── auth_settings.rs
└── state.rs

crates/shared/src/
├── config/
│   └── settings.rs
├── auth.rs               # JWT middleware (AuthenticatedUser / JwtVerification)
└── domain/
    ├── errors.rs
    └── value_objects/    # Email, HashedPassword, TokenPair, etc.

crates/infrastructure/src/
├── database.rs
├── rate_limiter/         # memory.rs, redis.rs
├── telemetry/            # logging.rs, metrics.rs, pollers.rs
└── rate_limiter_setup.rs

src/
├── main.rs               # composition root (migrations, seed, server start)
├── router.rs             # build_router
├── cors.rs               # build_cors_layer
├── api_doc.rs            # combined OpenAPI spec
├── health.rs
└── security_headers.rs
```

**Structure Decision**: Modular Rust monolith (DDD + hexagonal) as defined in the constitution. The `crates/auth` bounded context owns the auth domain, application ports/use-cases, and adapters; `crates/shared` holds config + cross-context value objects; `crates/infrastructure` provides db pool, telemetry, and rate limiters; the top-level `src/` crate is the composition root wiring routes, CORS, TLS, and the OpenAPI spec.

## Complexity Tracking

> All constitution gates pass — no violations to justify.
