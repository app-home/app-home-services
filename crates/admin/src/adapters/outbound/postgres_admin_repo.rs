use std::sync::Arc;

use async_trait::async_trait;
use shared::user_directory::UserDirectory;
use sqlx::PgPool;
use uuid::Uuid;

use crate::application::ports::admin_repository::AdminRepository;
use crate::domain::entities::admin_user::AdminUser;
use crate::domain::errors::AdminError;
use crate::domain::value_objects::role::Role;

/// Postgres implementation of `AdminRepository`.
///
/// Owns and queries `user_roles` directly (this context's own table, see migration
/// 008) but never queries `users` -- user identity fields come from `user_directory`
/// instead. This is the resolution of the coupling documented in
/// `docs/adr/0001-modular-monolith.md`: `admin` now depends only on the
/// `UserDirectory` port living in `shared`, not on `auth`'s crate or its `users`
/// table directly.
#[derive(Clone)]
pub struct PostgresAdminRepo {
    pool: PgPool,
    user_directory: Arc<dyn UserDirectory>,
}

impl PostgresAdminRepo {
    pub fn new(pool: PgPool, user_directory: Arc<dyn UserDirectory>) -> Self {
        Self {
            pool,
            user_directory,
        }
    }

    /// Looks up a single user's role from `user_roles`. A missing row means the
    /// user was never promoted and defaults to `Role::User` -- this mirrors the
    /// `DEFAULT 'user'` behavior the old `users.role` column had, so a user with no
    /// row here behaves exactly as one with an explicit `role = 'user'` row would.
    async fn role_for(&self, user_id: Uuid) -> Result<Role, AdminError> {
        let role_str = sqlx::query_scalar::<_, String>(
            "SELECT role FROM user_roles WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AdminError::InternalError(e.to_string()))?;

        match role_str {
            Some(s) => Role::try_from(s.as_str()).map_err(AdminError::InternalError),
            None => Ok(Role::User),
        }
    }

    /// Looks up every explicitly-assigned role in one query, returned as
    /// `(user_id, Role)` pairs. Used by `list_users` to avoid one `role_for` query
    /// per user. Users with no row here (never promoted) are simply absent from the
    /// result -- callers should default to `Role::User` for any user_id not present.
    async fn all_assigned_roles(&self) -> Result<Vec<(Uuid, Role)>, AdminError> {
        let rows = sqlx::query_as::<_, (Uuid, String)>("SELECT user_id, role FROM user_roles")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AdminError::InternalError(e.to_string()))?;

        rows.into_iter()
            .map(|(user_id, role_str)| {
                Role::try_from(role_str.as_str())
                    .map(|role| (user_id, role))
                    .map_err(AdminError::InternalError)
            })
            .collect()
    }
}

#[async_trait]
impl AdminRepository for PostgresAdminRepo {
    async fn list_users(&self) -> Result<Vec<AdminUser>, AdminError> {
        let summaries = self
            .user_directory
            .list_user_summaries()
            .await
            .map_err(|e| AdminError::InternalError(e.to_string()))?;

        let assigned_roles = self.all_assigned_roles().await?;
        let role_for_id = |id: Uuid| {
            assigned_roles
                .iter()
                .find(|(uid, _)| *uid == id)
                .map(|(_, role)| role.clone())
                .unwrap_or(Role::User)
        };

        Ok(summaries
            .into_iter()
            .map(|s| {
                let role = role_for_id(s.id);
                AdminUser::new(
                    s.id,
                    s.username,
                    s.email,
                    s.display_name,
                    role,
                    s.auth_provider,
                    s.created_at,
                    s.updated_at,
                )
            })
            .collect())
    }

    async fn get_user(&self, user_id: Uuid) -> Result<AdminUser, AdminError> {
        let summary = self
            .user_directory
            .get_user_summary(user_id)
            .await
            .map_err(|e| AdminError::InternalError(e.to_string()))?
            .ok_or(AdminError::NotFound(user_id))?;

        let role = self.role_for(user_id).await?;

        Ok(AdminUser::new(
            summary.id,
            summary.username,
            summary.email,
            summary.display_name,
            role,
            summary.auth_provider,
            summary.created_at,
            summary.updated_at,
        ))
    }

    async fn is_admin(&self, user_id: Uuid) -> Result<bool, AdminError> {
        // Confirm the user actually exists (mirrors the old behavior of NotFound
        // when the users row was missing) before reporting a role for them.
        let exists = self
            .user_directory
            .get_user_summary(user_id)
            .await
            .map_err(|e| AdminError::InternalError(e.to_string()))?
            .is_some();

        if !exists {
            return Err(AdminError::NotFound(user_id));
        }

        Ok(self.role_for(user_id).await?.is_admin())
    }

    async fn update_role(&self, user_id: Uuid, role: &Role) -> Result<AdminUser, AdminError> {
        sqlx::query(
            r#"INSERT INTO user_roles (user_id, role, updated_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (user_id) DO UPDATE SET role = EXCLUDED.role, updated_at = NOW()"#,
        )
        .bind(user_id)
        .bind(role.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| AdminError::InternalError(e.to_string()))?;

        self.get_user(user_id).await
    }
}
