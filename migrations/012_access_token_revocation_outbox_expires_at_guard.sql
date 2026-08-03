-- no-transaction
--
-- Safety net for migration 011. A CREATE INDEX CONCURRENTLY that fails partway
-- (e.g. is cancelled) leaves the index behind in an invalid state, and 011's
-- `IF NOT EXISTS` then silently reuses it on retry -- the index "exists" but
-- is unusable, so the flush worker's expiry sweep silently falls back to a
-- sequential scan. Detect that case via pg_index.indisvalid and rebuild.
--
-- This must be a single statement (a DO block): sqlx runs a `-- no-transaction`
-- migration as one simple-protocol query, which Postgres wraps in a single
-- implicit transaction, so CONCURRENTLY operations cannot coexist with any
-- other statement -- and they cannot run inside a DO block either. Rebuilding
-- with plain (non-concurrent) DDL here is safe: the index is already broken,
-- and the outbox table is normally empty (it only fills during a Redis outage,
-- see migration 009).
DO $migration$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        JOIN pg_index i ON i.indexrelid = c.oid
        WHERE n.nspname = 'public'
          AND c.relname = 'access_token_revocation_outbox_expires_at_idx'
          AND NOT i.indisvalid
    ) THEN
        EXECUTE 'DROP INDEX access_token_revocation_outbox_expires_at_idx';
        EXECUTE 'CREATE INDEX access_token_revocation_outbox_expires_at_idx
                 ON access_token_revocation_outbox (expires_at)';
    END IF;
END
$migration$;
