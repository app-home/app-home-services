use chrono::{DateTime, Utc};
use shared::domain::events::{
    Event, session_refreshed::SessionRefreshed, user_logged_in::UserLoggedIn,
    user_logged_out::UserLoggedOut,
};
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::hashed_password::HashedPassword;
use uuid::Uuid;

use crate::domain::entities::session::{NewSession, Session};
use crate::domain::entities::user::User;
use crate::domain::errors::AuthError;

const MAX_ACTIVE_SESSIONS: usize = 25;

pub struct UserAggregate {
    pub user: User,
    pub sessions: Vec<Session>,
    pending_events: Vec<Event>,
}

impl UserAggregate {
    pub fn new(user: User, sessions: Vec<Session>) -> Self {
        Self {
            user,
            sessions,
            pending_events: Vec::new(),
        }
    }

    pub fn user(&self) -> &User {
        &self.user
    }

    pub fn add_session(
        &mut self,
        session_id: Uuid,
        refresh_token_hash: HashedPassword,
        expires_at: DateTime<Utc>,
        auth_method: AuthMethod,
    ) -> Result<NewSession, AuthError> {
        let active = self
            .sessions
            .iter()
            .filter(|s| s.is_active() && !s.is_expired())
            .count();

        if active >= MAX_ACTIVE_SESSIONS {
            return Err(AuthError::InternalError(
                "maximum active sessions reached".into(),
            ));
        }

        let new_session = NewSession::new(
            session_id,
            self.user.id(),
            refresh_token_hash,
            expires_at,
            auth_method,
        );

        self.pending_events
            .push(Event::UserLoggedIn(UserLoggedIn::new(
                self.user.id(),
                session_id,
                auth_method,
            )));

        Ok(new_session)
    }

    pub fn invalidate_session(&mut self, session_id: Uuid) -> Result<AuthMethod, AuthError> {
        let session = self
            .sessions
            .iter_mut()
            .find(|s| s.id() == session_id)
            .ok_or(AuthError::SessionNotFound)?;

        if session.user_id() != self.user.id() {
            return Err(AuthError::SessionNotFound);
        }

        if !session.is_active() {
            return Err(AuthError::SessionInvalidated);
        }

        if session.is_expired() {
            return Err(AuthError::SessionExpired);
        }

        let auth_method = *session.auth_method();
        session.invalidate();

        self.pending_events
            .push(Event::UserLoggedOut(UserLoggedOut::new(
                self.user.id(),
                session_id,
                auth_method,
            )));

        Ok(auth_method)
    }

    pub fn rotate_session(
        &mut self,
        session_id: Uuid,
        refresh_token: &str,
        new_session_id: Uuid,
        new_refresh_hash: HashedPassword,
        expires_at: DateTime<Utc>,
    ) -> Result<NewSession, AuthError> {
        let session = self
            .sessions
            .iter_mut()
            .find(|s| s.id() == session_id)
            .ok_or(AuthError::SessionNotFound)?;

        if !session.is_active() {
            return Err(AuthError::SessionInvalidated);
        }

        if session.is_expired() {
            return Err(AuthError::SessionExpired);
        }

        let is_valid = match bcrypt::verify(refresh_token, session.refresh_token_hash().as_ref()) {
            Ok(valid) => valid,
            Err(e) => {
                tracing::error!(error = %e, "bcrypt::verify failed for refresh token hash");
                false
            }
        };
        if !is_valid {
            return Err(AuthError::InvalidRefreshToken);
        }

        let auth_method = *session.auth_method();
        session.invalidate();

        let new_session = NewSession::new(
            new_session_id,
            self.user.id(),
            new_refresh_hash,
            expires_at,
            auth_method,
        );

        self.pending_events
            .push(Event::SessionRefreshed(SessionRefreshed::new(
                self.user.id(),
                new_session_id,
                auth_method,
            )));

        Ok(new_session)
    }

    pub fn invalidate_all_active_sessions(&mut self) {
        for session in &mut self.sessions {
            if session.is_active() {
                let session_id = session.id();
                let auth_method = *session.auth_method();
                session.invalidate();
                self.pending_events
                    .push(Event::UserLoggedOut(UserLoggedOut::new(
                        self.user.id(),
                        session_id,
                        auth_method,
                    )));
            }
        }
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.pending_events)
    }
}
