#[derive(Debug, Clone)]
pub struct Settings {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub default_user_username: String,
    pub default_user_password: String,
    pub default_user_email: String,
    pub google_client_id: String,
    pub jwt_secret: String,
    pub rate_limit_max_attempts: u32,
    pub rate_limit_window_seconds: u64,
    pub access_token_expiry_minutes: i64,
    pub refresh_token_expiry_days: i64,
    pub cors_allowed_origins: String,
}

impl Settings {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| "DATABASE_URL must be set".to_string())?,
            server_host: std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
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
            google_client_id: std::env::var("GOOGLE_CLIENT_ID").unwrap_or_else(|_| String::new()),
            jwt_secret: std::env::var("JWT_SECRET")
                .map_err(|_| "JWT_SECRET must be set".to_string())?,
            rate_limit_max_attempts: std::env::var("RATE_LIMIT_MAX_ATTEMPTS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .map_err(|_| "RATE_LIMIT_MAX_ATTEMPTS must be a valid number".to_string())?,
            rate_limit_window_seconds: std::env::var("RATE_LIMIT_WINDOW_SECONDS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .map_err(|_| "RATE_LIMIT_WINDOW_SECONDS must be a valid number".to_string())?,
            access_token_expiry_minutes: std::env::var("ACCESS_TOKEN_EXPIRY_MINUTES")
                .unwrap_or_else(|_| "15".to_string())
                .parse()
                .map_err(|_| "ACCESS_TOKEN_EXPIRY_MINUTES must be a valid number".to_string())?,
            refresh_token_expiry_days: std::env::var("REFRESH_TOKEN_EXPIRY_DAYS")
                .unwrap_or_else(|_| "7".to_string())
                .parse()
                .map_err(|_| "REFRESH_TOKEN_EXPIRY_DAYS must be a valid number".to_string())?,
            cors_allowed_origins: std::env::var("CORS_ALLOWED_ORIGINS")
                .unwrap_or_else(|_| String::new()),
        })
    }
}
