<!--
  Sync Impact Report

  Version change: 1.0.0 → 1.1.0
  Modified principles:
    - "V. PostgreSQL Persistence Standards · VI. Observability and Reliability" →
      UUID primary key rule added under PostgreSQL Persistence Standards
  Added sections: None
  Removed sections: None
  Templates requiring updates:
    - .specify/templates/plan-template.md: ✅ No changes needed (generic Constitution Check)
    - .specify/templates/spec-template.md: ✅ No changes needed
    - .specify/templates/tasks-template.md: ✅ No changes needed
  Command files (.opencode/commands/): ✅ No outdated references
  Runtime guidance (README.md, AGENTS.md): ✅ No constitution references to update
  Follow-up TODOs: None
-->

# App Home Services Constitution

## Core Principles

### I. Hexagonal Architecture (NON-NEGOTIABLE)

The system must follow Hexagonal Architecture (Ports and Adapters).

The dependency direction must always point inward:

Adapters → Application → Domain

The Domain layer is the core of the system and must not depend on:
- Database implementations
- HTTP frameworks
- External services
- Infrastructure concerns

Business rules must live in the Domain and Application layers.

Infrastructure details must be replaceable through defined ports/interfaces.

---

### II. Domain-Driven Design Principles

The codebase must be organized around business capabilities and domain concepts.

Domain models must:
- Represent business rules explicitly.
- Avoid exposing infrastructure details.
- Protect their own invariants.

Use cases must represent application actions and orchestrate domain behavior.

Entities, Value Objects, Aggregates, and Domain Services should be created when they provide real business value.

---

### III. Rust-First Development

The project must use Rust as the primary programming language.

Requirements:

- Rust Edition 2024.
- Code must compile with zero warnings.
- Formatting must follow `cargo fmt`.
- Static analysis must pass with `cargo clippy`.
- Unsafe Rust must be avoided unless explicitly justified.

Error handling must be explicit:
- Use `Result` for recoverable failures.
- Use `thiserror` for domain/application errors.
- Avoid `unwrap()` and `expect()` in production code.

---

### IV. Test-First and Quality Gates (NON-NEGOTIABLE)

Tests are required for all business-critical functionality.

The development workflow follows:

1. Define expected behavior.
2. Create tests.
3. Implement functionality.
4. Refactor while keeping tests green.

Required test levels:

- Unit tests for domain rules.
- Integration tests for adapters.
- Database tests for persistence behavior.

Every feature must include validation criteria before implementation.

---

### V. PostgreSQL Persistence Standards · VI. Observability and Reliability

PostgreSQL is the primary relational database.

Database access must:
- Use SQLx.
- Use migrations for schema changes.
- Never modify production schemas manually.
- Keep SQL separated from business logic.
- Use UUID primary keys (id columns) for all tables, not auto-incrementing integers.

Database models must not leak into domain entities.

The persistence layer belongs in infrastructure adapters.

The application must provide operational visibility.

Requirements:
- Structured logging using `tracing`.
- Meaningful error messages.
- Correlation identifiers for distributed operations.
- Health checks for external dependencies.

Failures must be handled explicitly and must not silently disappear.

---

## Technology Constraints

The project technology stack is:

### Backend

- Rust
- Tokio async runtime
- Axum for HTTP APIs
- SQLx for PostgreSQL access

### Database

- PostgreSQL
- SQLx migrations

### Development Tools

- Cargo
- cargo fmt
- cargo clippy
- Automated tests

Additional dependencies must be justified according to project needs.

---

## Project Structure Standards

The project should follow this organization:

```
src/
├── domain/
│   ├── entities/
│   ├── value_objects/
│   └── errors.rs
│
├── application/
│   ├── use_cases/
│   ├── services/
│   └── ports/
│
├── adapters/
│   ├── inbound/
│   └── outbound/
│
├── infrastructure/
│   ├── database/
│   ├── config/
│   └── telemetry/
│
└── main.rs
```

The folder structure may evolve, but architectural boundaries must remain clear.

### Development Workflow

Every feature must follow the Spec Driven Development workflow:

1. Create feature specification.
2. Review requirements and acceptance criteria.
3. Generate implementation plan.
4. Generate development tasks.
5. Implement following the constitution rules.
6. Validate with tests and quality checks.

Feature specifications must define:
- Business objective.
- Functional requirements.
- Acceptance criteria.
- Constraints.

Implementation plans must define:
- Architectural changes.
- Components affected.
- Database changes.
- Testing strategy.

---

## Governance

This constitution defines the mandatory engineering rules of the project.

Any implementation, feature specification, or technical decision must comply with these principles.

Changes to this constitution require:
- Documentation of the motivation.
- Review of affected components.
- Migration plan if existing code is impacted.

The constitution has priority over individual implementation preferences.

All code reviews must verify compliance with:
- Architecture rules.
- Testing requirements.
- Security considerations.
- Quality standards.

Complexity must be justified. Prefer simple solutions that satisfy current requirements.

**Version**: 1.1.0 | **Ratified**: 2026-07-01 | **Last Amended**: 2026-07-01
