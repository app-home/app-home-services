ALTER TABLE sessions
    ADD COLUMN IF NOT EXISTS auth_method VARCHAR(20) NOT NULL DEFAULT 'password';

CREATE INDEX IF NOT EXISTS idx_sessions_auth_method ON sessions(auth_method);
