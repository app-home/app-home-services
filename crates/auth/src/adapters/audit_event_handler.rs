use chrono::Utc;
use shared::domain::events::Event;
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::event_type::EventType;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct AuditEventHandler {
    pool: PgPool,
}

impl AuditEventHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn handle(&self, event: Event) {
        let result = match &event {
            Event::UserLoggedIn(e) => {
                self.insert_action(e.user_id, Some(e.session_id), EventType::Login, e.method)
                    .await
            }
            Event::UserLoggedOut(e) => {
                self.insert_action(e.user_id, Some(e.session_id), EventType::Logout, e.method)
                    .await
            }
            Event::SessionRefreshed(e) => {
                self.insert_action(e.user_id, Some(e.session_id), EventType::Refresh, e.method)
                    .await
            }
            Event::UserCreated(_) => {
                // UserCreated events are informational, no user_action row needed
                Ok(())
            }
        };

        if let Err(e) = result {
            tracing::error!(error = %e, event_type = %event.event_type(), "Failed to persist audit event");
        }
    }

    async fn insert_action(
        &self,
        user_id: Uuid,
        session_id: Option<Uuid>,
        event_type: EventType,
        auth_method: AuthMethod,
    ) -> Result<(), sqlx::Error> {
        let id = Uuid::now_v7();
        let now = Utc::now();

        sqlx::query(
            r#"INSERT INTO user_actions (id, user_id, session_id, event_type, auth_method, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(id)
        .bind(user_id)
        .bind(session_id)
        .bind(event_type.as_str())
        .bind(auth_method.as_str())
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
