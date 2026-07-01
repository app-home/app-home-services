ALTER TABLE user_actions
    ADD COLUMN IF NOT EXISTS session_id UUID REFERENCES sessions(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS event_type VARCHAR(20) NOT NULL DEFAULT 'login';

CREATE INDEX IF NOT EXISTS idx_user_actions_session_id ON user_actions(session_id);
CREATE INDEX IF NOT EXISTS idx_user_actions_event_type ON user_actions(event_type);
