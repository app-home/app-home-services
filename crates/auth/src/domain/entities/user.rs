use chrono::{DateTime, Utc};
use shared::domain::value_objects::auth_provider::AuthProvider;
use shared::domain::value_objects::email::Email;
use shared::domain::value_objects::hashed_password::HashedPassword;
use tracing::error;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct User {
    id: Uuid,
    username: Option<String>,
    email: Email,
    display_name: String,
    password_hash: Option<HashedPassword>,
    auth_provider: AuthProvider,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl User {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Uuid,
        username: Option<String>,
        email: Email,
        display_name: String,
        password_hash: Option<HashedPassword>,
        auth_provider: AuthProvider,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self::validate_invariants(&password_hash, &auth_provider);
        Self {
            id,
            username,
            email,
            display_name,
            password_hash,
            auth_provider,
            created_at,
            updated_at,
        }
    }

    fn validate_invariants(password_hash: &Option<HashedPassword>, auth_provider: &AuthProvider) {
        match auth_provider {
            AuthProvider::Local => {
                debug_assert!(
                    password_hash.is_some(),
                    "Local user must have a password hash"
                );
            }
            AuthProvider::Google => {
                debug_assert!(
                    password_hash.is_none(),
                    "Google user must not have a password hash"
                );
            }
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    pub fn email(&self) -> &Email {
        &self.email
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn password_hash(&self) -> Option<&HashedPassword> {
        self.password_hash.as_ref()
    }

    pub fn auth_provider(&self) -> &AuthProvider {
        &self.auth_provider
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }

    pub fn verify_password(&self, password: &str) -> bool {
        match &self.password_hash {
            Some(hash) => match bcrypt::verify(password, hash.as_ref()) {
                Ok(valid) => valid,
                Err(e) => {
                    error!(error = %e, "bcrypt::verify failed for user password hash");
                    false
                }
            },
            None => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub username: Option<String>,
    pub email: Email,
    pub display_name: String,
    pub password_hash: Option<HashedPassword>,
    pub auth_provider: AuthProvider,
}

impl NewUser {
    pub fn new_local(
        username: Option<String>,
        email: Email,
        display_name: String,
        password_hash: HashedPassword,
    ) -> Self {
        Self {
            username,
            email,
            display_name,
            password_hash: Some(password_hash),
            auth_provider: AuthProvider::Local,
        }
    }

    pub fn new_google(email: Email, display_name: String) -> Self {
        Self {
            username: None,
            email,
            display_name,
            password_hash: None,
            auth_provider: AuthProvider::Google,
        }
    }
}
