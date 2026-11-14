use crate::{
    application::{commands::RegisterUserCommand, dto::UserDto, errors::IdentityApplicationError},
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
            IdentityDomainError::InvalidEmail(_) => {
                ApiError::bad_request("INVALID_EMAIL", domain_err.to_string())
            }
            IdentityDomainError::InvalidFullName => {
                ApiError::bad_request("INVALID_FULL_NAME", domain_err.to_string())
            }
            IdentityDomainError::InvalidPassword => {
                ApiError::bad_request("INVALID_PASSWORD", domain_err.to_string())
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
    payload: Result<Json<RegisterUserRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<UserDto>), ApiError> {
    let Json(payload) = payload.map_err(|rejection| match rejection {
        JsonRejection::MissingJsonContentType(_) => ApiError::bad_request(
            "INVALID_CONTENT_TYPE",
            "Request must use 'Content-Type: application/json' with fields: email, full_name",
        ),
        _ => ApiError::bad_request(
            "INVALID_JSON_PAYLOAD",
            "Invalid JSON payload. Expected JSON object with fields: email (string), full_name (string)",
        ),
    })?;

    let (email, full_name, password) = match (payload.email, payload.full_name, payload.password) {
        (Some(email), Some(full_name), Some(password)) => (email, full_name, password),
        (None, Some(_), Some(_)) => {
            return Err(ApiError::bad_request("MISSING_EMAIL", "Missing required field: email"))
        }
        (Some(_), None, Some(_)) => {
            return Err(ApiError::bad_request(
                "MISSING_FULL_NAME",
                "Missing required field: fullName",
            ))
        }
        (Some(_), Some(_), None) => {
            return Err(ApiError::bad_request(
                "MISSING_PASSWORD",
                "Missing required field: password",
            ))
        }
        _ => {
            return Err(ApiError::bad_request(
                "MISSING_REQUIRED_FIELDS",
                "Missing required fields: email, fullName, password",
            ))
        }
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
