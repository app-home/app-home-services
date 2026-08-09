<!--
  Sync Impact Report

  Version change: 1.2.0 → 1.2.1
  Modified principles: None
  Modified sections:
    - "Project Structure Standards" → composition-root (src/) list gains
      router.rs, cors.rs, and security_headers.rs, which were extracted out of
      main.rs after the last amendment (CORS #191, security headers #90, both
      merged); the shared-kernel listing corrects config.rs to its real
      config/settings.rs and adds net.rs; the bounded-context adapter shape now
      notes that outbound adapters may live flat under adapters/ (as in auth)
      rather than only in an outbound/ subdirectory.
  Removed sections: None
  Templates requiring updates:
    - .specify/templates/plan-template.md: ✅ No changes needed (generic
      Constitution Check, not structure-specific)
    - .specify/templates/spec-template.md: ✅ No changes needed
    - .specify/templates/tasks-template.md: ✅ No changes needed
  Command files (.opencode/commands/): ⚠ Not reviewed in this amendment -- if any
    command hardcodes the old single-crate src/ paths (src/domain/, src/adapters/,
    etc.) rather than crates/<context>/src/..., it should be updated to match.
  Runtime guidance (README.md, AGENTS.md) and stale-fact follow-ups:
    - AGENTS.md is auto-updated by opencode tooling and was refreshed in #194
      (spec-plan sync); its migration-007/008 admin claim remains as-the-monitor
      reports, not hand-edited here.
-->

# App Home Services Constitution

## Core Principles

### I. Hexagonal Architecture (NON-NEGOTIABLE)

Every bounded-context crate must follow Hexagonal Architecture (Ports and Adapters) internally.

The dependency direction must always point inward, within each crate:

Adapters → Application → Domain

The Domain layer is the core of each bounded context and must not depend on:
- Database implementations
- HTTP frameworks
- External services
- Infrastructure concerns
- Another bounded context's crate (see Principle VII)

Business rules must live in the Domain and Application layers.

Infrastructure details must be replaceable through defined ports/interfaces.

The composition root (`src/`) is exempt from this rule in one specific sense: it is
allowed to depend on every bounded-context crate, since its entire job is to wire
them together. It must not, however, contain Domain or Application logic of its
own -- see Principle VII and Project Structure Standards below.

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

### VII. Modular Monolith Boundaries (NON-NEGOTIABLE)

The system is organized as a modular monolith: one deployable binary, composed of
independent bounded-context crates in a single Cargo workspace. See
`docs/adr/0001-modular-monolith.md` for the full rationale, including why this is
preferred over separate services today and what extracting a context into its own
service later would require.

This principle exists to keep that extraction realistic. It requires:

- **One crate per bounded context.** Each bounded context (e.g. `auth`, `profiles`,
  `admin`) lives in its own crate under `crates/`, with its own `Cargo.toml`,
  following Hexagonal Architecture internally (Principle I).
- **No bounded-context crate may depend on another bounded-context crate.** A
  context's `Cargo.toml` may depend on `shared` and `infrastructure`, and nothing
  else in the workspace. This is enforced by the compiler once respected, and must
  be verified (not just assumed) whenever a new cross-context need arises -- check
  the actual `[dependencies]` block, not just the intent.
- **Cross-context communication happens only through ports defined in `shared`.**
  When one context needs something from another (identity lookups, domain events,
  etc.), the contract is a trait defined in `crates/shared/`, implemented by the
  owning context, and injected at the composition root (`src/main.rs`) as a trait
  object (e.g. `Arc<dyn Trait>`). A context must never call into another context's
  concrete types or query another context's database tables directly. (Example:
  `admin` reads user identity through `shared::user_directory::UserDirectory`,
  implemented by `auth`, rather than depending on the `auth` crate or querying
  `auth`'s `users` table.)
- **Each bounded context owns its own data.** A context's tables belong to that
  context. A foreign key into another context's table for referential integrity is
  acceptable (both contexts share one database today), but a context must not read
  or write columns that another context owns. If two contexts appear to need the
  same piece of data, decide which one owns it and expose it to the other through a
  port, rather than both querying the same table directly.
- **The composition root wires, it does not implement.** `src/main.rs` (and
  `src/lib.rs`) may construct concrete adapters and inject them into other
  contexts' ports, and may combine per-context OpenAPI specs (`src/api_doc.rs`),
  but must not contain Domain or Application logic belonging to any bounded
  context.

Any change that would violate one of these rules (a new `path = "../other-context"`
dependency between two bounded-context crates, a raw SQL query against a table
owned by a different context, business logic added directly to `src/`) requires
either restructuring the change to go through a port, or an explicit amendment to
this constitution documenting why the exception is justified -- per Governance
below.

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

The project is a Cargo workspace: one crate per bounded context, a shared kernel, a
cross-cutting infrastructure crate, and a thin composition-root binary. See
Principle VII for the rules this structure exists to enforce, and
`docs/modules/*.md` for what each crate currently owns.

```
Cargo.toml                  # workspace manifest
src/                        # composition root (binary crate)
├── main.rs                 # wires every context together, starts the server
├── lib.rs                  # thin re-export layer
├── router.rs               # build_router: route wiring + conditional SwaggerUi mount
├── cors.rs                 # build_cors_layer
├── security_headers.rs     # HTTP security headers (HSTS, X-Content-Type-Options, …)
├── health.rs
└── api_doc.rs              # combined OpenAPI spec across all bounded contexts

crates/
├── shared/                 # shared kernel -- leaf dependency, no workspace deps
│   └── src/
│       ├── domain/         # DomainError, cross-context value objects + events
│       ├── ports.rs        # cross-context ports (e.g. RateLimiter)
│       ├── user_directory.rs  # cross-context ports with their own DTOs
│       ├── event_bus.rs    # async pub/sub for cross-context domain events
│       ├── auth.rs         # AuthenticatedUser JWT extractor
│       ├── api.rs          # shared API types (ErrorResponse, etc.)
│       ├── net.rs          # trusted-proxy / network helpers
│       └── config/
│           └── settings.rs # infra-level Settings
│
├── infrastructure/         # cross-cutting, depends only on shared
│   └── src/                # db pool, telemetry (logging/metrics), rate limiter setup,
│                           #   access-token blacklist (memory/redis)
│
├── <bounded-context>/      # one per context, e.g. auth/, profiles/, admin/
│   └── src/
│       ├── domain/
│       │   ├── entities/
│       │   ├── value_objects/
│       │   └── errors.rs
│       ├── application/
│       │   ├── use_cases/
│       │   ├── ports/
│       │   └── (services/, where useful)
│       ├── adapters/
│       │   ├── inbound/   # HTTP handlers, request/response DTOs
│       │   └── outbound/  # ports implementations (or flat under adapters/, as in auth)
│       ├── config/          # context-specific settings, where needed
│       └── lib.rs
│
└── ...                      # future bounded contexts follow the same shape
```

Each bounded-context crate's internal folder structure may evolve, but the
Adapters → Application → Domain boundary within it (Principle I) and the
no-cross-context-crate-dependency rule (Principle VII) must remain clear and
enforced. Outbound adapters (database repos, external services) may be grouped
under an `outbound/` subdirectory or kept directly in `adapters/` (as the `auth`
crate does today); the distinction that matters is that they remain adapters
below the application layer and are never reached from other contexts.

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
- Architectural changes, including which bounded-context crate(s) are affected and
  whether any new cross-context port is needed (Principle VII).
- Components affected.
- Database changes, including which bounded context owns any new table or column.
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
- Architecture rules, including bounded-context boundaries (Principle VII).
- Testing requirements.
- Security considerations.
- Quality standards.

Complexity must be justified. Prefer simple solutions that satisfy current requirements.

**Version**: 1.2.1 | **Ratified**: 2026-07-01 | **Last Amended**: 2026-08-09
