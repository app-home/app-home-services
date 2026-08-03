-- no-transaction
--
-- Safety net for migration 013. REINDEX INDEX CONCURRENTLY is the documented
-- recovery for an index left invalid by migration 011's CREATE INDEX
-- CONCURRENTLY, but if that reindex itself fails partway, PostgreSQL leaves a
-- transient invalid index behind whose name is the target index suffixed with
-- `_ccnew` (or `_ccold` after a partially completed swap), plus a nonzero
-- number when the base suffix name is already taken (e.g. `_ccnew1`). Per the
-- PostgreSQL docs the recovery is to DROP INDEX those leftovers before
-- retrying the concurrent reindex, and they would otherwise keep consuming
-- update overhead forever.
--
-- This migration drops every invalid index named
-- `access_token_revocation_outbox_expires_at_idx_cc%`, then migration 013
-- retries the concurrent rebuild in the same run. Plain DROP INDEX is fine
-- here (no CONCURRENTLY): the leftovers are invalid, so nothing uses them and
-- dropping them is quick.
--
-- This must be a single statement (a DO block): sqlx runs a `-- no-transaction`
-- migration as one simple-protocol query, and REINDEX/CONCURRENTLY cannot
-- coexist with other statements inside that implicit transaction anyway -- so
-- the drop lives here and the concurrent reindex lives on its own in 013.
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
          AND c.relname LIKE 'access_token_revocation_outbox_expires_at_idx_cc%'
          AND NOT i.indisvalid
    LOOP
        EXECUTE format('DROP INDEX %I.%I', r.nspname, r.relname);
    END LOOP;
END
$migration$;
