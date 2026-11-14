use identity::{
    application::dto::UserDto,
    presentation::http::{handlers::__path_register_user_handler, requests::RegisterUserRequest},
};
use shared_kernel::presentation::ProblemDetails;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Rust DDD Modular Monolith API",
        version = "0.1.0",
        description = "Domain-Driven Design (DDD) Backend in Rust with Identity Bounded Context, CQRS RegisterUser Use-Case, and Domain Event Bus."
    ),
    paths(
        register_user_handler,
    ),
    components(
        schemas(
            RegisterUserRequest,
            UserDto,
            ProblemDetails,
        )
    ),
    tags(
        (name = "Identity Context", description = "User registration")
    )
)]
pub struct ApiDoc;
