-- Index the expiry sweep for DurableRevocationBlacklist::flush_pending.
--
-- Migration 009 originally tried an expression index on
-- `created_at + (ttl_secs * INTERVAL '1 second')` so the flush worker's
-- `DELETE ... WHERE created_at + (ttl_secs * INTERVAL '1 second') <= NOW()`
-- could use an index scan. Postgres rejects it: `timestamptz + interval` is
-- STABLE, not IMMUTABLE (error 42P17), and there is no immutable expression of
-- a timestamptz that Postgres will index -- even `extract(epoch FROM ...)`
-- lowers to `date_part(timestamptz, ...)`, which is STABLE too.
--
-- Instead, compute the expiry once at insert time into a plain `expires_at`
-- column, which a normal b-tree index can serve. Journal writes set it to
-- `NOW() + ttl_secs * INTERVAL '1 second'`; the flush worker's expiry sweep
-- becomes `DELETE ... WHERE expires_at <= NOW()`. A row's lifetime is decided
-- when it is journaled and never changes afterwards (`ON CONFLICT DO NOTHING`
-- keeps the first write), so the column is stable.
DROP INDEX IF EXISTS access_token_revocation_outbox_expires_at_idx;

ALTER TABLE access_token_revocation_outbox
    ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ;

-- Backfill for rows journaled under the pre-expires_at schema.
UPDATE access_token_revocation_outbox
   SET expires_at = created_at + (ttl_secs * INTERVAL '1 second')
 WHERE expires_at IS NULL;

ALTER TABLE access_token_revocation_outbox
    ALTER COLUMN expires_at SET NOT NULL;

CREATE INDEX IF NOT EXISTS access_token_revocation_outbox_expires_at_idx
    ON access_token_revocation_outbox (expires_at);
