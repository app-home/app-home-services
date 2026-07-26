use std::time::Duration;

use axum::Extension;
use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use sqlx::PgPool;

use shared::api::HealthResponse;

/// Timeout for the health check's own database probe. Deliberately short: a health
/// check that takes a long time to fail is not much better than one that doesn't
/// check anything -- callers (load balancers, k8s probes) have their own timeouts
/// and are better served by a fast, definitive answer.
const HEALTH_CHECK_DB_TIMEOUT: Duration = Duration::from_secs(2);

#[utoipa::path(
    get,
    path = "/api/health",
    tag = "Health",
    responses(
        (status = 200, description = "Service healthy: database reachable", body = HealthResponse),
        (status = 503, description = "Service degraded: database unreachable or the check timed out", body = HealthResponse),
    ),
)]
pub async fn health_check(Extension(pool): Extension<PgPool>) -> impl IntoResponse {
    // A trivial query against the pool, rather than just checking the pool object
    // exists: this actually exercises acquiring a connection and round-tripping to
    // Postgres, so a genuinely unreachable/overloaded database is caught, not just
    // a pool that was successfully constructed at startup.
    let db_check = tokio::time::timeout(
        HEALTH_CHECK_DB_TIMEOUT,
        sqlx::query("SELECT 1").execute(&pool),
    )
    .await;

    match db_check {
        Ok(Ok(_)) => (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            }),
        )
            .into_response(),
        Ok(Err(e)) => {
            tracing::error!(error = %e, "Health check: database query failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "degraded".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                }),
            )
                .into_response()
        }
        Err(_) => {
            tracing::error!(
                timeout_secs = HEALTH_CHECK_DB_TIMEOUT.as_secs(),
                "Health check: database query timed out"
            );
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "degraded".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                }),
            )
                .into_response()
        }
    }
}
