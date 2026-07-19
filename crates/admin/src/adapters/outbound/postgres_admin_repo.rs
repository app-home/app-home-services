use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::application::ports::admin_repository::AdminRepository;
use crate::domain::entities::admin_user::AdminUser;
use crate::domain::errors::AdminError;
use crate::domain::value_objects::role::Role;

#[derive(Clone)]
pub struct PostgresAdminRepo {
    pool: PgPool,
}

impl PostgresAdminRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AdminRepository for PostgresAdminRepo {
    async fn list_users(&self) -> Result<Vec<AdminUser>, AdminError> {
        let rows = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT id, username, email, display_name, role, auth_provider, created_at, updated_at
            FROM users
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AdminError::InternalError(e.to_string()))?;

        rows.into_iter()
            .map(|r| {
                let role = Role::try_from(r.role.as_str()).map_err(AdminError::InternalError)?;
                Ok(AdminUser::new(
                    r.id,
                    r.username,
                    r.email,
                    r.display_name,
                    role,
                    r.auth_provider,
                    r.created_at,
                    r.updated_at,
                ))
            })
            .collect()
    }

    async fn get_user(&self, user_id: Uuid) -> Result<AdminUser, AdminError> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT id, username, email, display_name, role, auth_provider, created_at, updated_at
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AdminError::InternalError(e.to_string()))?
        .ok_or(AdminError::NotFound(user_id))?;

        let role = Role::try_from(row.role.as_str()).map_err(AdminError::InternalError)?;

        Ok(AdminUser::new(
            row.id,
            row.username,
            row.email,
            row.display_name,
            role,
            row.auth_provider,
            row.created_at,
            row.updated_at,
        ))
    }

    async fn is_admin(&self, user_id: Uuid) -> Result<bool, AdminError> {
        let row = sqlx::query_scalar::<_, String>(
            r#"
            SELECT role FROM users WHERE id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AdminError::InternalError(e.to_string()))?
        .ok_or(AdminError::NotFound(user_id))?;

        Ok(row == "admin")
    }

    async fn update_role(&self, user_id: Uuid, role: &Role) -> Result<AdminUser, AdminError> {
        let role_str = role.as_str();
        sqlx::query("UPDATE users SET role = $1 WHERE id = $2")
            .bind(role_str)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| AdminError::InternalError(e.to_string()))?;

        self.get_user(user_id).await
    }
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    username: Option<String>,
    email: String,
    display_name: String,
    role: String,
    auth_provider: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}
