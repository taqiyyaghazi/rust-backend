use identity::{
    application::dto::{AuthResponseDto, TokenPairDto, UserDto},
    presentation::http::{
        handlers::{
            __path_login_user_handler, __path_logout_user_handler, __path_me_handler,
            __path_refresh_token_handler, __path_register_user_handler,
        },
        requests::{LoginRequest, LogoutRequest, RefreshTokenRequest, RegisterUserRequest},
    },
};
use shared_kernel::presentation::ProblemDetails;
use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

struct BearerSecurityAddon;

impl Modify for BearerSecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Rust DDD Modular Monolith API",
        version = "0.1.0",
        description = "Domain-Driven Design (DDD) Backend in Rust with Identity Bounded Context and Bearer JWT + Refresh Token Authentication."
    ),
    paths(
        register_user_handler,
        login_user_handler,
        refresh_token_handler,
        logout_user_handler,
        me_handler,
    ),
    components(
        schemas(
            RegisterUserRequest,
            LoginRequest,
            RefreshTokenRequest,
            LogoutRequest,
            UserDto,
            AuthResponseDto,
            TokenPairDto,
            ProblemDetails,
        )
    ),
    modifiers(&BearerSecurityAddon),
    tags(
        (name = "Identity Context", description = "User registration, authentication, and session lifecycle")
    )
)]
pub struct ApiDoc;
