use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpBuilder, SecurityScheme},
};

use auth::adapters::inbound::health_routes::__path_health_check;
use auth::adapters::inbound::login_routes::{__path_login_password_handler, PasswordLoginRequest};
use auth::adapters::inbound::logout_routes::{__path_logout_handler, LogoutRequest};
use auth::adapters::inbound::oauth_callback::{__path_login_google_handler, GoogleLoginRequest};
use auth::adapters::inbound::refresh_routes::{__path_refresh_token_handler, RefreshTokenRequest};
use auth::adapters::inbound::responses::{
    AuthTokensResponse, ErrorResponse, GoogleAuthResponse, HealthResponse, RefreshResponse,
    StatusResponse,
};

use admin::adapters::inbound::admin_routes::{
    __path_get_user_handler, __path_list_users_handler, __path_update_user_role_handler,
};
use admin::adapters::inbound::responses::{UpdateRoleRequest, UserResponse};
use profiles::adapters::inbound::profile_routes::{
    __path_get_profile_handler, __path_update_profile_handler,
};
use profiles::adapters::inbound::responses::{ProfileResponse, UpdateProfileRequest};

#[derive(OpenApi)]
#[openapi(
    info(description = "App Home Services API"),
    tags(
        (name = "Authentication", description = "Authentication & session management"),
        (name = "Profiles", description = "User profile management"),
        (name = "Admin", description = "Admin user management"),
    ),
    servers(
        (url = "http://localhost:3000", description = "Local development"),
    ),
    paths(
        login_password_handler,
        login_google_handler,
        logout_handler,
        refresh_token_handler,
        health_check,
        get_profile_handler,
        update_profile_handler,
        list_users_handler,
        get_user_handler,
        update_user_role_handler,
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
        ProfileResponse,
        UpdateProfileRequest,
        UserResponse,
        UpdateRoleRequest,
    )),
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi
            .components
            .as_mut()
            .expect("ApiDoc defines components/schemas, so this should never be None");
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
