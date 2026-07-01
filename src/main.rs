use axum::{Router, routing::post};

use app_home_services::adapters::inbound::login_routes::login_password_handler;
use app_home_services::adapters::inbound::oauth_callback::login_google_handler;
use app_home_services::adapters::outbound::google_auth_provider::GoogleAuthProvider;
use app_home_services::adapters::outbound::postgres_user_repo::PostgresUserRepo;
use app_home_services::infrastructure::config::settings::Settings;
use app_home_services::AppState;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    app_home_services::infrastructure::telemetry::logging::init_logging();

    tracing::info!("Starting App Home Services");

    let settings = Settings::from_env().expect("Failed to load settings");

    let pool = app_home_services::infrastructure::database::db::create_pool(&settings.database_url)
        .await
        .expect("Failed to create database pool");

    app_home_services::infrastructure::database::db::run_migrations(&pool)
        .await
        .expect("Failed to run database migrations");

    seed_default_user(&pool, &settings).await;

    let user_repo = PostgresUserRepo::new(pool);
    let auth_provider = GoogleAuthProvider::new(settings.google_client_id.clone());

    let state = AppState::new(user_repo, auth_provider, settings);

    let app = Router::new()
        .route("/api/auth/login/password", post(login_password_handler))
        .route("/api/auth/login/google", post(login_google_handler))
        .route("/api/health", axum::routing::get(health_check))
        .with_state(state);

    let addr = format!("{}:{}", "0.0.0.0", 3000);
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}

async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({"status": "ok"}))
}

async fn seed_default_user(pool: &sqlx::PgPool, settings: &Settings) {
    let existing = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE auth_provider = 'local'")
        .fetch_one(pool)
        .await;

    match existing {
        Ok(0) | Err(_) => {
            let password_hash = bcrypt::hash(&settings.default_user_password, bcrypt::DEFAULT_COST)
                .expect("Failed to hash default password");

            let id = uuid::Uuid::now_v7();
            let now = chrono::Utc::now();

            let result = sqlx::query(
                r#"INSERT INTO users (id, username, email, display_name, password_hash, auth_provider, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, 'local', $6, $7)
                ON CONFLICT (username) DO NOTHING"#,
            )
            .bind(id)
            .bind(&settings.default_user_username)
            .bind(&settings.default_user_email)
            .bind("Administrator")
            .bind(&password_hash)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await;

            match result {
                Ok(_) => tracing::info!("Default user seeded successfully"),
                Err(e) => tracing::error!(error = %e, "Failed to seed default user"),
            }
        }
        Ok(_) => tracing::info!("Default user already exists, skipping seed"),
    }
}
