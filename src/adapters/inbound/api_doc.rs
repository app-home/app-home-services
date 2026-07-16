use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpBuilder, SecurityScheme},
};

use crate::adapters::inbound::health_routes::__path_health_check;
use crate::adapters::inbound::login_routes::{__path_login_password_handler, PasswordLoginRequest};
use crate::adapters::inbound::logout_routes::{__path_logout_handler, LogoutRequest};
use crate::adapters::inbound::oauth_callback::{__path_login_google_handler, GoogleLoginRequest};
use crate::adapters::inbound::refresh_routes::{__path_refresh_token_handler, RefreshTokenRequest};
use crate::adapters::inbound::responses::{
    AuthTokensResponse, ErrorResponse, GoogleAuthResponse, HealthResponse, RefreshResponse,
    StatusResponse,
};

#[derive(OpenApi)]
#[openapi(
    info(description = "Authentication service API"),
    servers(
        (url = "http://localhost:3000", description = "Local development"),
    ),
    paths(
        login_password_handler,
        login_google_handler,
        logout_handler,
        refresh_token_handler,
        health_check,
    ),
    components(schemas(
        PasswordLoginRequest,
        GoogleLoginRequest,
        LogoutRequest,
        RefreshTokenRequest,
        AuthTokensResponse,
        GoogleAuthResponse,
        RefreshResponse,
        StatusResponse,
        HealthResponse,
        ErrorResponse,
    )),
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.as_mut().unwrap();
        components.add_security_scheme(
            "bearer_jwt",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .description(Some("JWT Bearer token authentication"))
                    .build(),
            ),
        );
    }
}
