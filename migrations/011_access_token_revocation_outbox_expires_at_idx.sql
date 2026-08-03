-- no-transaction
--
-- CREATE INDEX CONCURRENTLY cannot run inside a transaction, so this migration
-- is marked to skip sqlx's default per-migration transaction wrapper. It must
-- stay a single statement for that reason -- do not add other schema changes
-- to this file.
--
-- CONCURRENTLY avoids taking the write lock that a plain CREATE INDEX would
-- hold on access_token_revocation_outbox for the duration of the build,
-- which matters once the durable-revocation flush worker (see #140) is
-- actively inserting/deleting rows against this table in production.
CREATE INDEX CONCURRENTLY IF NOT EXISTS access_token_revocation_outbox_expires_at_idx
    ON access_token_revocation_outbox (expires_at);
