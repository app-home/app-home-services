-- The admin context previously stored `role` as a column on `users` (migration 007),
-- a table owned by the auth context. That was the strongest cross-context coupling
-- in the codebase: every admin repository method ran SQL directly against `users`.
--
-- This migration gives `admin` a table it actually owns. `user_id` keeps a foreign
-- key into `users` for referential integrity (both contexts still share one
-- database today), but `admin`'s application code no longer queries `users`
-- directly -- user identity now comes through the `UserDirectory` port implemented
-- by `auth` (see docs/adr/0001-modular-monolith.md).
--
-- A user with no row here defaults to role 'user' -- this mirrors the DEFAULT
-- 'user' the `role` column had on `users`, so behavior is unchanged for anyone who
-- was never explicitly promoted.
CREATE TABLE IF NOT EXISTS user_roles (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL DEFAULT 'user' CHECK (role IN ('user', 'admin')),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Backfill: carry over every existing non-default role so no admin loses their role
-- in this migration. Users who were never promoted (role = 'user') don't need a row
-- here at all -- the application layer treats "no row" as role 'user'.
INSERT INTO user_roles (user_id, role)
SELECT id, role FROM users WHERE role <> 'user';

ALTER TABLE users DROP COLUMN role;
