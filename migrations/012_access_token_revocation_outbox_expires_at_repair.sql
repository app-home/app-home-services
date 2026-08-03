-- no-transaction
--
-- Recovery guard for migration 011. CREATE INDEX CONCURRENTLY that fails partway
-- (e.g. is cancelled) leaves access_token_revocation_outbox_expires_at_idx
-- behind in an INVALID state, and 011's `IF NOT EXISTS` then silently reuses it
-- on retry -- the index "exists" but is unusable, so the flush worker's expiry
-- sweep silently falls back to a sequential scan. A REINDEX CONCURRENTLY retry
-- can in turn fail partway and leave transient invalid indexes named
-- `<target>_ccnew` (or `<target>_ccold` after a partially completed swap, with a
-- nonzero number appended when the base suffix name is already taken, e.g.
-- `_ccnew1`).
--
-- This migration is the self-healing retry unit: sqlx only records a migration
-- as applied after it succeeds, so this DO block runs again on every retry after
-- a failure. It:
--
--  1. Drops every invalid `_ccnew`/`_ccold` leftover, so a retry never trips over
--     them (per the PostgreSQL docs the recovery for those is plain DROP INDEX).
--  2. Only then, if the target index is invalid, rebuilds it with a plain
--     (non-concurrent) `REINDEX INDEX`. It never touches a valid index, so
--     migration 011's normal path costs nothing.
--
-- Both the DROP INDEX and the plain (non-concurrent) rebuild can briefly block
-- reads and writes on the outbox table: DROP INDEX takes an ACCESS EXCLUSIVE
-- lock on the parent table, and a non-concurrent REINDEX blocks writes and
-- virtually all queries against it, including sequential scans. This is
-- acceptable because this branch only runs when the index is already broken
-- (queries ignore it and scan anyway) and the outbox table is normally empty
-- (it only fills during a Redis outage, see migration 009). REINDEX INDEX
-- CONCURRENTLY cannot be used here: it cannot run inside a DO block or share
-- the migration's single implicit transaction, and sqlx runs a
-- `-- no-transaction` migration as one simple-protocol query -- so a
-- conditional/concurrent rebuild is impossible.
DO $migration$
DECLARE
    r record;
BEGIN
    FOR r IN
        SELECT n.nspname, c.relname
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        JOIN pg_index i ON i.indexrelid = c.oid
        WHERE n.nspname = 'public'
          AND c.relname ~ '^access_token_revocation_outbox_expires_at_idx_cc(new|old)[0-9]*$'
          AND NOT i.indisvalid
    LOOP
        EXECUTE format('DROP INDEX %I.%I', r.nspname, r.relname);
    END LOOP;

    IF EXISTS (
        SELECT 1
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        JOIN pg_index i ON i.indexrelid = c.oid
        WHERE n.nspname = 'public'
          AND c.relname = 'access_token_revocation_outbox_expires_at_idx'
          AND NOT i.indisvalid
    ) THEN
        EXECUTE 'REINDEX INDEX public.access_token_revocation_outbox_expires_at_idx';
    END IF;
END
$migration$;
