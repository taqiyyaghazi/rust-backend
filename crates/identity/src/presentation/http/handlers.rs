use crate::{
    application::{
        commands::RegisterUserCommand,
        dto::AuthResponseDto,
        errors::IdentityApplicationError,
    },
    domain::errors::IdentityDomainError,
    presentation::{http::requests::RegisterUserRequest, state::IdentityHttpState},
};
use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    Json,
};
use shared_kernel::presentation::{ApiError, ProblemDetails};

fn map_identity_error(err: IdentityApplicationError) -> ApiError {
    match err {
        IdentityApplicationError::Domain(domain_err) => match domain_err {
            IdentityDomainError::InvalidEmail(_) => ApiError::bad_request("INVALID_EMAIL"),
            IdentityDomainError::InvalidFullName => ApiError::bad_request("INVALID_FULL_NAME"),
            IdentityDomainError::InvalidPassword => ApiError::bad_request("INVALID_PASSWORD"),
        },
        IdentityApplicationError::EmailAlreadyExists(_) => ApiError::conflict("EMAIL_ALREADY_EXISTS"),
        IdentityApplicationError::PasswordHashing | IdentityApplicationError::TokenGeneration => {
            tracing::error!("Authentication internal error");
            ApiError::internal()
        }
        IdentityApplicationError::InvalidCredentials => {
            ApiError::unauthorized("INVALID_CREDENTIALS")
        }
        IdentityApplicationError::InvalidRefreshToken => {
            ApiError::unauthorized("INVALID_REFRESH_TOKEN")
        }
        IdentityApplicationError::InvalidToken => ApiError::unauthorized("INVALID_TOKEN"),
        IdentityApplicationError::Repository(repo_err) => {
            tracing::error!(error = %repo_err, "Identity repository error");
            ApiError::internal()
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
        (status = 201, description = "User registered successfully", body = AuthResponseDto),
        (status = 400, description = "Domain validation error", body = ProblemDetails),
        (status = 409, description = "Email already registered", body = ProblemDetails)
    )
)]
pub async fn register_user_handler(
    State(state): State<IdentityHttpState>,
    payload: Result<Json<RegisterUserRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<AuthResponseDto>), ApiError> {
    let Json(payload) = payload.map_err(|rejection| match rejection {
        JsonRejection::MissingJsonContentType(_) => ApiError::bad_request("INVALID_CONTENT_TYPE"),
        JsonRejection::BytesRejection(r) if r.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            ApiError::payload_too_large()
        }
        _ => ApiError::bad_request("INVALID_JSON_PAYLOAD"),
    })?;

    let (email, full_name, password) = match (payload.email, payload.full_name, payload.password) {
        (Some(email), Some(full_name), Some(password)) => (email, full_name, password),
        (None, Some(_), Some(_)) => return Err(ApiError::bad_request("MISSING_EMAIL")),
        (Some(_), None, Some(_)) => return Err(ApiError::bad_request("MISSING_FULL_NAME")),
        (Some(_), Some(_), None) => return Err(ApiError::bad_request("MISSING_PASSWORD")),
        _ => return Err(ApiError::bad_request("MISSING_REQUIRED_FIELDS")),
    };

    let user = state
        .register_user
        .handle(RegisterUserCommand {
            email,
            full_name,
            password,
        })
        .await
        .map_err(map_identity_error)?;

    Ok((StatusCode::CREATED, Json(user)))
}
