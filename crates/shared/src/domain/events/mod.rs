pub mod session_refreshed;
pub mod user_created;
pub mod user_logged_in;
pub mod user_logged_out;

use chrono::{DateTime, Utc};
use uuid::Uuid;

pub use session_refreshed::SessionRefreshed;
pub use user_created::UserCreated;
pub use user_logged_in::UserLoggedIn;
pub use user_logged_out::UserLoggedOut;

use super::value_objects::auth_method::AuthMethod;
#[derive(Debug, Clone)]
pub enum Event {
    UserLoggedIn(UserLoggedIn),
    UserLoggedOut(UserLoggedOut),
    SessionRefreshed(SessionRefreshed),
    UserCreated(UserCreated),
}

impl Event {
    pub fn event_type(&self) -> &'static str {
        match self {
            Event::UserLoggedIn(_) => "user_logged_in",
            Event::UserLoggedOut(_) => "user_logged_out",
            Event::SessionRefreshed(_) => "session_refreshed",
            Event::UserCreated(_) => "user_created",
        }
    }

    pub fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Event::UserLoggedIn(e) => e.occurred_at,
            Event::UserLoggedOut(e) => e.occurred_at,
            Event::SessionRefreshed(e) => e.occurred_at,
            Event::UserCreated(e) => e.occurred_at,
        }
    }

    pub fn aggregate_id(&self) -> Uuid {
        match self {
            Event::UserLoggedIn(e) => e.user_id,
            Event::UserLoggedOut(e) => e.user_id,
            Event::SessionRefreshed(e) => e.user_id,
            Event::UserCreated(e) => e.user_id,
        }
    }

    pub fn auth_method(&self) -> Option<AuthMethod> {
        match self {
            Event::UserLoggedIn(e) => Some(e.method),
            Event::UserLoggedOut(e) => Some(e.method),
            Event::SessionRefreshed(e) => Some(e.method),
            Event::UserCreated(_) => None,
        }
    }

    pub fn session_id(&self) -> Option<Uuid> {
        match self {
            Event::UserLoggedIn(e) => Some(e.session_id),
            Event::UserLoggedOut(e) => Some(e.session_id),
            Event::SessionRefreshed(e) => Some(e.session_id),
            Event::UserCreated(_) => None,
        }
    }

    pub fn user_id(&self) -> Uuid {
        self.aggregate_id()
    }
}
