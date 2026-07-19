# Plan: User Profiles Bounded Context

## Goal

Extract a new `profiles` bounded context for managing user profile data,
following the same DDD + hexagonal architecture as `auth`.

## Completed

- [x] Migration 006: `user_profiles` table
- [x] Crate structure: `crates/profiles/`
- [x] Domain: `Profile` entity, `AvatarUrl`, `Bio` value objects, `ProfileError`
- [x] Port: `ProfileRepository` trait
- [x] Use cases: `get_profile`, `update_profile`
- [x] Adapter: `PostgresProfileRepo`
- [x] Inbound routes: `GET /api/profile`, `PUT /api/profile`
- [x] Combined OpenAPI spec in `src/api_doc.rs`
- [x] Contracts for GET/PUT /api/profile
