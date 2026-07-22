# ADR 0001: Modular monolith, not microservices (yet)

- **Status:** Accepted
- **Date:** 2026-07-20
- **Related:** #77, PRs #71–#75 (the migration itself)

## Context

This project started as a single crate with hexagonal architecture (`src/domain`,
`src/application`, `src/adapters`) implementing one bounded context: authentication.
As `profiles` and `admin` were added, the code was restructured into a **Cargo
workspace**: one crate per bounded context (`crates/auth`, `crates/profiles`,
`crates/admin`), a shared kernel (`crates/shared`), a cross-cutting infrastructure
crate (`crates/infrastructure`), and a thin composition-root binary (`src/`) that
wires everything together and serves one HTTP process.

That restructuring is done and merged (see `docs/modules/*.md` for what each crate
owns). This document records **why** that shape was chosen over the alternative --
splitting `auth`, `profiles`, and `admin` into separately deployed services from the
start -- and gives a concrete, verified account of what stands between today's
structure and being able to pull a context out later.

## Decision

Keep bounded contexts as workspace crates inside one deployable binary, with Rust's
crate boundaries enforcing separation, rather than deploying each context as its own
service today.

### 1. Why a modular monolith and not microservices today

- **One binary, one deploy.** At the current scale (one maintainer, low/moderate
  traffic), running N separate services means N CI/CD pipelines, N sets of
  credentials, N observability surfaces, and a network hop between contexts that
  didn't exist before. That's real, ongoing operational cost with no corresponding
  benefit yet -- nothing here needs to scale independently or ship on its own
  release cycle right now.
- **Transactions stay simple.** `auth`, `profiles`, and `admin` all read and write
  the same PostgreSQL database today (see "Where the real coupling lives" below).
  Splitting into services before that changes would mean immediately having to
  solve distributed consistency (sagas, eventual consistency, dual writes) for a
  problem that doesn't exist yet -- the data is, today, genuinely relational and
  benefits from being queried as such.
- **The boundary cost is paid once, cheaply.** Splitting into crates gets most of a
  microservice's isolation benefit -- the compiler enforces that nothing outside a
  crate touches its private internals -- without paying for the network, contract
  versioning between services, or multi-service deployment infrastructure. A crate
  boundary is nearly free to introduce and nearly free to widen later; a service
  boundary, once wrong, is expensive to redraw.

### 2. The actual benefit: incremental extraction, and where it's blocked today

The reason to invest in this structure now, instead of waiting until it's needed, is
that **when a context genuinely needs to scale independently, extracting it should be
a bounded, low-risk change -- not a rewrite.** That claim is only worth something if
it's checked against the real code, not asserted. Here's what's actually true today:

**What already supports a clean extraction:**

- No bounded-context crate depends on another. Verified directly from each crate's
  `Cargo.toml`: `auth`, `profiles`, and `admin` each depend on `shared` (and, where
  needed, `infrastructure`) and nothing else in the workspace. `shared` itself
  depends on no other workspace crate -- it's the leaf of the graph
  (`docs/modules/shared.md`).
- Cross-context authentication is already decoupled from any specific crate.
  `profiles` and `admin` authenticate requests via `shared::AuthenticatedUser`, an
  Axum extractor that decodes a JWT using a `DecodingKey` handed to the router as an
  `Extension` in `main.rs`. Neither `profiles` nor `admin` depends on the `auth`
  crate's types, session store, or JWT service to do this.
- There's already a precedent for decoupled, asynchronous cross-context
  communication: `shared::EventBus` (a `tokio::sync::broadcast` channel) carries
  domain events (`UserLoggedIn`, `UserLoggedOut`, `SessionRefreshed`, `UserCreated`)
  from `auth` to an `AuditEventHandler`, without either side depending on the
  other's internals. This is the pattern an extracted service would lean on more
  heavily (in-process broadcast today, a message queue or webhook once one side is
  a separate process).

**What would actually block a clean extraction today, and would need to be resolved
first:**

- **`admin` reads and writes `auth`'s table directly.** `crates/admin/src/adapters/outbound/postgres_admin_repo.rs`
  runs raw SQL against the `users` table for all four of its repository methods
  (`list_users`, `get_user`, `is_admin`, `update_role`). `admin` doesn't own its own
  table -- it owns a *column* (`role`, added in migration 007) on a table that
  `auth` created and conceptually owns (migration 001). This is the single largest
  piece of real coupling in the codebase: extracting `admin` today would mean either
  giving the extracted service direct access to `auth`'s database (defeating much of
  the point of extraction) or building a new API in `auth` for `admin` to call
  instead of querying the table directly.
- **`profiles` has a foreign key into `auth`'s table.** `user_profiles.user_id`
  references `users(id) ON DELETE CASCADE` (migration 006). This is a lighter form
  of coupling than `admin`'s -- `profiles`' own repository code
  (`postgres_profile_repo.rs`) never queries the `users` table directly, only
  `user_profiles` -- but the referential-integrity guarantee ("a profile can't exist
  for a user that doesn't exist, and disappears when the user does") is currently
  enforced by Postgres, not by application code. An extracted `profiles` service
  would need to either replicate `auth`'s user existence data locally or give up
  that guarantee at the database level and enforce it in application code instead
  (e.g. by reacting to a `UserCreated`/`UserDeleted` event).
- **One shared `PgPool`.** `main.rs` creates a single connection pool and passes a
  clone of it into every context's repository constructor. This isn't a design flaw
  at the current stage, but it does mean "the database" isn't yet partitioned along
  bounded-context lines the way the code is -- extraction work for any context
  includes deciding what portion of the schema goes with it.

### 3. How to extract a crate into its own service, when it's warranted

1. **Confirm the crate has no direct dependency on another bounded-context crate.**
   True today for all three (`auth`, `profiles`, `admin`) -- verify this hasn't
   regressed before starting.
2. **Resolve any direct cross-context table access.** For `admin` specifically, this
   means giving `auth` an API (in-process function call today, HTTP/gRPC after
   extraction) for "list users" / "get user" / "check role" / "update role", and
   having `admin` call that instead of running SQL against `users` directly. This is
   the single most important item on this list, and can (and should) be done
   *before* any service-splitting work starts, as its own low-risk change.
3. **Resolve cross-context foreign keys.** For `profiles`, decide whether the
   extracted service keeps a local, eventually-consistent copy of user IDs (updated
   via the existing `EventBus` pattern, extended with `UserCreated`/`UserDeleted`
   handlers) or drops the DB-level FK and accepts a small window where a profile
   could outlive its user.
4. **Introduce a network interface for the extracted context, still in the same
   binary.** Swap the in-process call from step 2 for an HTTP (or gRPC) client, but
   keep both sides deployed together. This isolates "did the interface change work"
   from "did the deployment split work" as two separate, independently-verifiable
   changes.
5. **Split the schema.** Decide which tables/columns move with the extracted
   context and which stay; write the migration; update the data-ownership section of
   that context's `docs/modules/<context>.md`.
6. **Give the extracted crate its own deployable: repository (or a separate
   binary/CI job in this one), its own `Cargo.toml` as a binary crate, and its own
   CI pipeline** (a working reference already exists at `.github/workflows/ci.yml`).
7. **Update `docs/modules/<context>.md`** to describe the new deployment shape and
   the network contract that replaced the in-process call.

### 4. Signals that extraction is actually warranted

Not exhaustive, but concrete enough to anchor a real decision rather than "it feels
like it's time":

- A context needs to scale horizontally independently of the others (e.g. `auth`
  under heavy login traffic while `admin` sees a handful of requests a day).
- A context needs a different technology (language, datastore) that no longer makes
  sense to force into this Rust/Postgres binary.
- A context gets a distinct release cadence or owning team, and the shared deploy
  starts blocking one side's changes on the other's readiness.

## Consequences

- **Positive:** Rust's crate system gives most of the isolation benefit of service
  boundaries today, at compile time, for free. The dependency graph is already
  clean (no bounded context depends on another). Cross-context auth and
  audit-logging already go through decoupled mechanisms (`shared::AuthenticatedUser`,
  `shared::EventBus`) rather than direct calls into another context's internals.
- **Negative / accepted trade-off:** `admin`'s direct SQL access to `users` is real
  coupling that exists today and would need deliberate work (step 2 above) before
  `admin` could be extracted cleanly. This ADR doesn't schedule that work -- it's
  listed here so it isn't rediscovered by surprise later.
- **Negative / accepted trade-off:** All contexts currently share one `PgPool` and
  one Postgres database with no schema-level partitioning. Fine at the current
  scale; the first real extraction will have to reckon with it.
