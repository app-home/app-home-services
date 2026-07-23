# Data Model: Admin

## Overview

The admin bounded context provides user management capabilities for
administrator users. It owns its own `user_roles` table (added in migration 008,
replacing an earlier design that stored `role` as a column on `users`) and reads
user identity fields (username, email, display_name, auth_provider) through the
`UserDirectory` port defined in `shared` and implemented by the `auth` context --
`admin` does not query `users` directly. See
[`docs/adr/0001-modular-monolith.md`](../../docs/adr/0001-modular-monolith.md) for
why this shape was chosen.

## Table: `user_roles` (owned by `admin`)

| Column       | Type          | Constraints                                    |
|--------------|---------------|-------------------------------------------------|
| `user_id`    | `uuid`        | PK, FK -> `users.id` (`ON DELETE CASCADE`)      |
| `role`       | `varchar(20)` | NOT NULL, default `'user'`, CHECK in ('user','admin') |
| `updated_at` | `timestamptz` | NOT NULL, default `NOW()`                       |

A user with **no row** in this table has an implicit role of `'user'` -- rows are
only created here when a user is explicitly promoted (or demoted back, which
still writes a row) via `update_role`. This mirrors the `DEFAULT 'user'` behavior
the old `users.role` column had.

## Identity fields (via `UserDirectory`, not a local table)

`admin` combines a `user_roles` lookup with a `UserSummary` (from
`shared::user_directory::UserDirectory`, implemented by
`auth::adapters::postgres_user_directory::PostgresUserDirectory`) to build the
`AdminUser` entity returned to callers:

| Field           | Source                          |
|-----------------|----------------------------------|
| `id`            | `UserDirectory` (`users.id`)     |
| `username`      | `UserDirectory` (`users.username`) |
| `email`         | `UserDirectory` (`users.email`)  |
| `display_name`  | `UserDirectory` (`users.display_name`) |
| `auth_provider` | `UserDirectory` (`users.auth_provider`) |
| `created_at`    | `UserDirectory` (`users.created_at`) |
| `updated_at`    | `UserDirectory` (`users.updated_at`) |
| `role`          | `admin`'s own `user_roles` table (or `'user'` if no row) |

## Domain

| Value Object | Values                        |
|--------------|-------------------------------|
| `Role`       | `"user"` or `"admin"`         |
