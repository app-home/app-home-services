# Implementation Plan: Audit & Security Hardening

**Branch**: `feat/3-user-authentication` | **Date**: 2026-07-01 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/002-audit-security-hardening/spec.md`

## Summary

Extend the audit trail to track both login and logout events, add session-based authentication with JWT tokens and explicit logout, harden the startup process to fail on DB errors, and implement security hardening (rate limiting, timing-safe responses, restricted CORS).

## Technical Context

**Language/Version**: Rust (Edition 2024)

**Primary Dependencies**: Axum (HTTP), Tokio (async runtime), SQLx (PostgreSQL), `jsonwebtoken` (JWT), `bcrypt` (password hashing), `tracing` (logging), `tower-http` (CORS middleware), `serde`/`serde_json`, `chrono`, `uuid`, `thiserror`, `dotenvy`

**Storage**: PostgreSQL (via SQLx migrations)

**Testing**: `cargo test` (unit tests for session domain, rate limiter; integration tests for logout, refresh, CORS, startup)

**Target Platform**: Linux server (web service)

**Project Type**: web-service

**Performance Goals**: Login/logout/refresh flows complete within 2 seconds for 95% of requests under normal load

**Constraints**: UUID primary keys for all tables, hexagonal architecture (domain must not depend on infrastructure), no plain-text passwords in logs/responses, startup must abort on DB errors, CORS denied by default

**Scale/Scope**: Single-admin-user internal service; rate limiting in-memory per IP

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

**I. Hexagonal Architecture (NON-NEGOTIABLE)**
- PASS: Session entity in domain layer, SessionRepository/JwtService/RateLimiter ports in application, PostgresSessionRepo/JwtService/MemoryRateLimiter adapters in adapters layer.

**II. Domain-Driven Design**
- PASS: Session entity with business rules (active/inactive state, expiry validation). Logout/refresh use cases orchestrate domain behavior.

**III. Rust-First Development**
- PASS: Rust Edition 2024, cargo fmt, cargo clippy, zero warnings. Result-based error handling with thiserror.

**IV. Test-First and Quality Gates (NON-NEGOTIABLE)**
- PASS: Unit tests for Session validation and RateLimiter logic. Integration tests for logout HTTP, token refresh, CORS, and startup hardening.

**V. PostgreSQL Persistence Standards**
- PASS: New migrations for sessions table and user_actions extension. UUID primary keys throughout.

**VI. Observability and Reliability**
- PASS: Structured tracing for login/logout/refresh events and startup outcomes. Health check endpoint.

**GATE RESULT**: All gates pass. No complexity justification needed.

## Project Structure

### Documentation (this feature)

```text
specs/002-audit-security-hardening/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
└── tasks.md             # Phase 2 output
```

### Source Code (repository root)

```text
src/
├── domain/
│   ├── entities/
│   │   ├── user.rs              # Existing
│   │   ├── user_action.rs       # Extended (event_type, session_id)
│   │   ├── session.rs           # NEW
│   │   └── mod.rs
│   └── errors.rs                # Extended (SessionExpired, RateLimited, etc.)
│
├── application/
│   ├── use_cases/
│   │   ├── login_with_password.rs    # Modified (returns session)
│   │   ├── login_with_google.rs      # Modified (returns session)
│   │   ├── record_audit_entry.rs     # Extended (event_type, session_id)
│   │   ├── logout.rs                 # NEW
│   │   ├── refresh_token.rs          # NEW
│   │   └── mod.rs
│   └── ports/
│       ├── user_repository.rs        # Extended
│       ├── session_repository.rs     # NEW
│       ├── jwt_service.rs            # NEW
│       ├── rate_limiter.rs           # NEW
│       └── mod.rs
│
├── adapters/
│   ├── inbound/
│   │   ├── login_routes.rs           # Modified (returns tokens, rate-limited)
│   │   ├── oauth_callback.rs         # Modified (returns tokens)
│   │   ├── logout_routes.rs          # NEW
│   │   ├── refresh_routes.rs         # NEW
│   │   ├── auth_middleware.rs        # NEW
│   │   └── mod.rs
│   └── outbound/
│       ├── postgres_user_repo.rs     # Extended
│       ├── postgres_session_repo.rs  # NEW
│       ├── jwt_service.rs            # NEW
│       ├── memory_rate_limiter.rs    # NEW
│       ├── google_auth_provider.rs   # Existing
│       └── mod.rs
│
├── infrastructure/
│   ├── database/
│   │   ├── db.rs                    # Existing
│   │   └── mod.rs
│   ├── config/
│   │   └── settings.rs              # Extended (new env vars)
│   └── telemetry/
│       └── logging.rs               # Existing
│
├── lib.rs                           # Extended (AppState)
└── main.rs                          # Extended (new routes, CORS, hardened startup)

migrations/
├── 001_create_users_table.sql       # Existing
├── 002_create_user_actions_table.sql# Existing
├── 003_create_sessions_table.sql    # NEW
└── 004_extend_user_actions.sql      # NEW

tests/
├── unit/
│   ├── user_test.rs                 # Existing
│   ├── password_test.rs             # Existing
│   ├── audit_test.rs                # Existing
│   ├── session_test.rs              # NEW
│   └── rate_limiter_test.rs         # NEW
└── integration/
    ├── database_test.rs             # Existing
    ├── login_password_test.rs       # Existing (update)
    ├── login_google_test.rs         # Existing (update)
    ├── logout_test.rs               # NEW
    ├── refresh_test.rs              # NEW
    ├── rate_limit_test.rs           # NEW
    ├── cors_test.rs                 # NEW
    └── startup_test.rs              # NEW
```

**Structure Decision**: Single Rust project following existing Hexagonal Architecture. New entities and adapters follow the same patterns as existing code.

## Complexity Tracking

> All constitution gates pass — no violations to justify.
