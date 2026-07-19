# Plan: Admin Bounded Context

## Goal

Create an admin bounded context for user management, with role-based
access control using a `role` column on the `users` table.

## Completed

- [x] Migration 007: `role` column on `users` table
- [x] Crate structure: `crates/admin/`
- [x] Domain: `AdminUser` entity, `Role` value object, `AdminError`
- [x] Port: `AdminRepository` trait
- [x] Use cases: `list_users`, `get_user`, `update_user_role`
- [x] Adapter: `PostgresAdminRepo`
- [x] Inbound routes: `GET /api/admin/users`, `GET /api/admin/users/{id}`, `PUT /api/admin/users/{id}/role`
- [x] Combined OpenAPI spec in `src/api_doc.rs`
- [x] Contracts for admin endpoints
