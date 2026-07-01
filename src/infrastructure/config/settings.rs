#[derive(Debug, Clone)]
pub struct Settings {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub default_user_username: String,
    pub default_user_password: String,
    pub default_user_email: String,
    pub google_client_id: String,
}

impl Settings {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| "DATABASE_URL must be set".to_string())?,
            server_host: std::env::var("SERVER_HOST")
                .unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: std::env::var("SERVER_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .map_err(|_| "SERVER_PORT must be a valid number".to_string())?,
            default_user_username: std::env::var("DEFAULT_USER_USERNAME")
                .unwrap_or_else(|_| "admin".to_string()),
            default_user_password: std::env::var("DEFAULT_USER_PASSWORD")
                .map_err(|_| "DEFAULT_USER_PASSWORD must be set".to_string())?,
            default_user_email: std::env::var("DEFAULT_USER_EMAIL")
                .unwrap_or_else(|_| "admin@example.com".to_string()),
            google_client_id: std::env::var("GOOGLE_CLIENT_ID")
                .unwrap_or_else(|_| String::new()),
        })
    }
}
