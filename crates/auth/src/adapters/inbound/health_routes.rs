use axum::Json;

use crate::adapters::inbound::responses::HealthResponse;

#[utoipa::path(
    get,
    path = "/api/health",
    tag = "Health",
    responses(
        (status = 200, description = "Service healthy", body = HealthResponse),
    ),
)]
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
    })
}
