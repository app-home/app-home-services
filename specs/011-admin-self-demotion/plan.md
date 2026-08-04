# Implementation Plan: Prevent admin self-demotion

**Branch**: fix/92-admin-self-demotion
**Date**: 2026-08-04
**Issue**: #92

## Summary

Issue #92 (security, Medium) reports that `PUT /api/admin/users/{id}/role`
allows an admin to change their **own** role (e.g. demote themselves to `user`
to evade admin-only restrictions, or "disappear" from the admin list). There is
no self-demotion prevention.

Decisions (confirmed):
- **Scope: self-demotion prevention only.** The issue's optional "at least one
  admin" business rule is deferred.
- **Enforced at the use-case layer** (`update_user_role`), not the handler:
  the rule is a business invariant and becomes directly testable with the
  existing `MockAdminRepo`. The handler passes `auth_user.user_id` as the actor.

## WP A — Domain + use case (HIGH)

**Files**: `crates/admin/src/domain/errors.rs`,
`crates/admin/src/application/use_cases/update_user_role.rs`

- New `AdminError::CannotChangeOwnRole` variant
  (`"Cannot change your own role"`).
- `update_user_role(repo, actor_id, user_id, new_role)` returns
  `Err(CannotChangeOwnRole)` when `actor_id == user_id`, before any validation
  or repository write.

## WP B — HTTP handler (HIGH)

**Files**: `crates/admin/src/adapters/inbound/admin_routes.rs`

- Handler passes `auth_user.user_id` as `actor_id`.
- New match arm maps `AdminError::CannotChangeOwnRole` → `403 Forbidden`
  with `{"error": "Cannot change your own role"}`.

## WP C — Tests (MEDIUM)

**Files**: `crates/admin/tests/admin_test.rs`

- Existing `update_user_role_*` tests updated to pass a distinct `actor_id`.
- New cases: `update_user_role_rejects_self_demotion` and
  `update_user_role_rejects_self_promotion` (self-change rejected for either
  direction), plus coverage that another admin can still promote/demote.

## WP D — Contracts + plan (LOW)

**Files**: `specs/006-admin/contracts/update-user-role.md`,
`specs/011-admin-self-demotion/plan.md` (this file), `AGENTS.md`

- Contract documents the new 403 "Self Role Change" response and notes the
  rule (see #92). The `markdown_contract_consistency` test already accepts it
  (403 is in the generated spec).
- SPECKIT pointer in `AGENTS.md` updated to this plan.

## Validation

- `cargo build --locked`, `cargo clippy --workspace --all-targets`,
  `cargo test --locked -p admin`, `cargo test --locked --workspace` — all green.
- Contract-consistency and OpenAPI coverage tests unaffected.

## Notes

- Issue #92 auto-closes only when the PR reaches `main`, so a
  `development` → `main` promotion is required at the end.
