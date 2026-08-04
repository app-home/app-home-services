# Implementation Plan: Dependency security in CI (cargo-audit + cargo-deny)

**Branch**: fix/91-cargo-audit
**Date**: 2026-08-04
**Issue**: #91

## Summary

Issue #91 asked for CVE tracking of the Cargo dependency tree: a `cargo audit`
step in CI, a Dependabot config for Cargo, and optional `cargo-deny`. The
Dependabot half is **already in place** (`.github/dependabot.yml` — Cargo weekly
into `development` with a `patch-and-minor` group, plus github-actions monthly;
it already produced merged PRs #158/#159), so the remaining work is adding
`cargo-audit` and `cargo-deny` to CI. Both new checks are **informational** (per
decision): they run on every PR but are not required checks on the `main`
ruleset, which stays untouched.

## WP A — `cargo audit` in CI (HIGH)

**File**: `.github/workflows/ci.yml`

- New `security-audit` job ("Security audit (cargo-audit)"): `actions/checkout@v7`
  → `dtolnay/rust-toolchain@stable` → `actions/cache@v6` → `cargo install
  cargo-audit --locked && cargo audit`.
- `cargo audit` scans `Cargo.lock` against the RustSec advisory DB and exits
  non-zero when a CVE applies. Current state: **0 vulnerabilities** (1
  `unsound` warning: `event-listener 5.4.1`, RUSTSEC-2026-0221, warning only,
  exit 0).
- Inherits the workflow triggers (push to `main` + every PR).

## WP B — `cargo deny` (HIGH)

**Files**: `deny.toml` (new), `.github/workflows/ci.yml`

- New `deny.toml` (cargo-deny config, `version = 2` semantics — anything not in
  the `allow` list is rejected):
  - `[licenses]` `allow`: the exact SPDX set present in the lockfile
    (`0BSD`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`,
    `BSD-3-Clause`, `BSL-1.0`, `CC0-1.0`, `CDLA-Permissive-2.0`, `ISC`,
    `LGPL-2.1-or-later`, `MIT`, `MIT-0`, `Unicode-3.0`, `Unlicense`, `Zlib`).
    `LGPL-2.1-or-later` is allowed because its only user is `r-efi` (low-level
    EFI bindings, transitive, unmodified); strong copyleft (GPL/AGPL/GFDL/SSPL)
    is rejected by omission.
  - `[licenses.private] ignore = true` + `publish = false` on all six packages:
    the workspace crates carry no LICENSE file, so they are marked private and
    unpublished instead of inventing a license. `unlicensed = "deny"` for
    everything else.
  - `[bans]` `multiple-versions = "warn"` (~25 duplicated crates in the
    lockfile: tokio, chrono, uuid, thiserror, ... — report, don't fail, until
    Dependabot/toolchain updates dedupe them); `wildcards = "deny"` once every
    path dependency gets an explicit `version = "0.1"` (path-only deps
    otherwise count as `*`).
  - `[advisories]` defaults retained; the CI job runs `cargo deny check
    licenses bans` so the RustSec DB is only fetched once (by `cargo audit`).
- New `security-deny` job ("License + duplicate checks (cargo-deny)"): same
  checkout/toolchain/cache preamble → `cargo install cargo-deny --locked &&
  cargo deny check licenses bans`.

## WP C — Workspace metadata (MEDIUM)

**Files**: `Cargo.toml`, `crates/{admin,auth,shared,infrastructure,profiles}/Cargo.toml`

- `publish = false` on all six packages (they are internal, never published;
  this is what lets `licenses.private.ignore` scope out their license checks).
- `version = "0.1"` added to every path dependency so `wildcards = "deny"` holds.

## WP D — Plan document (LOW)

**Files**: `specs/009-cargo-audit/plan.md` (this file), `AGENTS.md`

- Plan versioned in the repo following the 007/008 convention.
- SPECKIT pointer in `AGENTS.md` updated to `specs/009-cargo-audit/plan.md`.

## Validation

- Local: `cargo audit` exit 0; `cargo deny check licenses bans` exit 0.
- `cargo build --locked` (lockfile still fresh after the Cargo.toml edits),
  `cargo clippy --workspace --all-targets`, `cargo test --lib` — no Rust code
  changed, so these must remain green.
- CI: PR to `development` → the two new jobs green.

## Notes

- The checks are informational by decision; the `main` ruleset is untouched.
- Issue #91 auto-closes only when the PR reaches `main` (same as #85/#86), so a
  `development` → `main` promotion is required at the end.
- Faster alternatives exist (`rustsec/audit-check@v2`, `taiki-e/install-action`)
  but were skipped to keep the changes self-contained and match the issue's
  literal proposal.
