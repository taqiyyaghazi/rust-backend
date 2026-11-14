use crate::{
    application::{commands::RegisterUserCommand, dto::UserDto, errors::IdentityApplicationError},
    domain::errors::IdentityDomainError,
    presentation::{http::requests::RegisterUserRequest, state::IdentityHttpState},
};
use axum::{extract::State, http::StatusCode, Json};
use shared_kernel::presentation::{ApiError, ProblemDetails};

fn map_identity_error(err: IdentityApplicationError) -> ApiError {
    match err {
        IdentityApplicationError::Domain(domain_err) => match domain_err {
            IdentityDomainError::InvalidEmail(_) | IdentityDomainError::InvalidFullName => {
                ApiError::bad_request("IDENTITY_VALIDATION_ERROR", domain_err.to_string())
            }
        },
        IdentityApplicationError::EmailAlreadyExists(email) => ApiError::conflict(
            "EMAIL_ALREADY_EXISTS",
            format!("Email '{}' is already registered", email),
        ),
        IdentityApplicationError::Repository(repo_err) => {
            tracing::error!(error = %repo_err, "Identity repository error");
            ApiError::internal("An internal error occurred")
        }
    }
}

/// Register a new User in the Identity Bounded Context.
#[utoipa::path(
    post,
    path = "/api/v1/register",
    tag = "Identity Context",
    request_body = RegisterUserRequest,
    responses(
        (status = 201, description = "User registered successfully", body = UserDto),
        (status = 400, description = "Domain validation error", body = ProblemDetails),
        (status = 409, description = "Email already registered", body = ProblemDetails)
    )
)]
pub async fn register_user_handler(
    State(state): State<IdentityHttpState>,
    Json(payload): Json<RegisterUserRequest>,
) -> Result<(StatusCode, Json<UserDto>), ApiError> {
    let user = state
        .register_user
        .handle(RegisterUserCommand {
            email: payload.email,
            full_name: payload.full_name,
        })
        .await
        .map_err(map_identity_error)?;

    Ok((StatusCode::CREATED, Json(user)))
}
