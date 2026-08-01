-- Durable retry for access token revocation (see #140).
--
-- The access token revocation list lives in Redis (`acl:revoked:<jti>`, shared
-- across instances, keyed by the token's remaining lifetime). Revocation only
-- ever needs to *succeed* in Redis; every authenticated request reads it. But a
-- logout must not silently lose a revocation just because Redis happens to be
-- down at that moment.
--
-- This table is the durable side of that guarantee. When Redis rejects a revoke
-- (see `DurableRevocationBlacklist` in crates/infrastructure), the pending
-- revocation is journaled here first so the logout still succeeds, and a
-- background flush worker (spawned from `main`) retries it against Redis until
-- it lands. Once it does, the row is deleted.
--
-- A row whose token lifetime has already elapsed (`created_at + ttl_secs <=
-- now`) is dropped without ever touching Redis -- the token it refers to can't
-- validate anymore anyway, so there is nothing left to revoke.
--
-- Owned by the infrastructure crate (not any single bounded context), since
-- every authenticated route's `AuthenticatedUser` extractor depends on it.
CREATE TABLE IF NOT EXISTS access_token_revocation_outbox (
    jti UUID PRIMARY KEY,
    -- Remaining lifetime of the token at revocation time (its blacklist TTL).
    ttl_secs BIGINT NOT NULL CHECK (ttl_secs >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- FIFO sweep order for the flush worker, and a way to cheaply find rows whose
-- `created_at + ttl_secs` has already elapsed without scanning the whole table.
CREATE INDEX IF NOT EXISTS access_token_revocation_outbox_created_at_idx
    ON access_token_revocation_outbox (created_at);
