use crate::{
    application::{
        commands::{
            LoginUserCommand, LogoutUserCommand, RefreshTokenCommand, RegisterUserCommand,
        },
        dto::{AuthResponseDto, TokenPairDto, UserDto},
        errors::IdentityApplicationError,
        queries::GetCurrentUserQuery,
    },
    domain::{errors::IdentityDomainError, value_objects::UserId},
    presentation::{
        http::requests::{
            LoginRequest, LogoutRequest, RefreshTokenRequest, RegisterUserRequest,
        },
        state::IdentityHttpState,
    },
};
use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    Json,
};
use shared_kernel::presentation::{ApiError, AuthenticatedUser, ProblemDetails};

fn map_identity_error(err: IdentityApplicationError) -> ApiError {
    match err {
        IdentityApplicationError::Domain(domain_err) => match domain_err {
            IdentityDomainError::InvalidEmail(_) => ApiError::bad_request("INVALID_EMAIL"),
            IdentityDomainError::InvalidFullName => ApiError::bad_request("INVALID_FULL_NAME"),
            IdentityDomainError::InvalidPassword => ApiError::bad_request("INVALID_PASSWORD"),
        },
        IdentityApplicationError::EmailAlreadyExists(_) => ApiError::conflict("EMAIL_ALREADY_EXISTS"),
        IdentityApplicationError::InvalidCredentials => {
            ApiError::unauthorized("INVALID_CREDENTIALS")
        }
        IdentityApplicationError::InvalidRefreshToken => {
            ApiError::unauthorized("INVALID_REFRESH_TOKEN")
        }
        IdentityApplicationError::InvalidToken => ApiError::unauthorized("INVALID_TOKEN"),
        IdentityApplicationError::PasswordHashing | IdentityApplicationError::TokenGeneration => {
            tracing::error!("Authentication internal error");
            ApiError::internal()
        }
        IdentityApplicationError::Repository(repo_err) => {
            tracing::error!(error = %repo_err, "Identity repository error");
            ApiError::internal()
        }
    }
}

fn extract_json<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    payload
        .map(|Json(v)| v)
        .map_err(|rejection| match rejection {
            JsonRejection::MissingJsonContentType(_) => {
                ApiError::bad_request("INVALID_CONTENT_TYPE")
            }
            JsonRejection::BytesRejection(r) if r.status() == StatusCode::PAYLOAD_TOO_LARGE => {
                ApiError::payload_too_large()
            }
            _ => ApiError::bad_request("INVALID_JSON_PAYLOAD"),
        })
}

fn require_non_empty(value: Option<String>, error_code: &'static str) -> Result<String, ApiError> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::bad_request(error_code))
}

/// Register a new User in the Identity Bounded Context and issue initial tokens.
#[utoipa::path(
    post,
    path = "/api/v1/register",
    tag = "Identity Context",
    request_body = RegisterUserRequest,
    responses(
        (status = 201, description = "User registered and authenticated", body = AuthResponseDto),
        (status = 400, description = "Domain validation error", body = ProblemDetails),
        (status = 409, description = "Email already registered", body = ProblemDetails)
    )
)]
pub async fn register_user_handler(
    State(state): State<IdentityHttpState>,
    payload: Result<Json<RegisterUserRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<AuthResponseDto>), ApiError> {
    let payload = extract_json(payload)?;

    let (email, full_name, password) = match (payload.email, payload.full_name, payload.password) {
        (Some(email), Some(full_name), Some(password)) => (email, full_name, password),
        (None, Some(_), Some(_)) => return Err(ApiError::bad_request("MISSING_EMAIL")),
        (Some(_), None, Some(_)) => return Err(ApiError::bad_request("MISSING_FULL_NAME")),
        (Some(_), Some(_), None) => return Err(ApiError::bad_request("MISSING_PASSWORD")),
        _ => return Err(ApiError::bad_request("MISSING_REQUIRED_FIELDS")),
    };

    let auth = state
        .register_user
        .handle(RegisterUserCommand {
            email,
            full_name,
            password,
        })
        .await
        .map_err(map_identity_error)?;

    Ok((StatusCode::CREATED, Json(auth)))
}

/// Authenticate an existing User with email and password.
#[utoipa::path(
    post,
    path = "/api/v1/login",
    tag = "Identity Context",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponseDto),
        (status = 400, description = "Validation error", body = ProblemDetails),
        (status = 401, description = "Invalid credentials", body = ProblemDetails)
    )
)]
pub async fn login_user_handler(
    State(state): State<IdentityHttpState>,
    payload: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<AuthResponseDto>), ApiError> {
    let payload = extract_json(payload)?;

    let (email, password) = match (payload.email, payload.password) {
        (Some(email), Some(password)) => (email, password),
        (None, Some(_)) => return Err(ApiError::bad_request("MISSING_EMAIL")),
        (Some(_), None) => return Err(ApiError::bad_request("MISSING_PASSWORD")),
        (None, None) => return Err(ApiError::bad_request("MISSING_REQUIRED_FIELDS")),
    };

    let auth = state
        .login_user
        .handle(LoginUserCommand { email, password })
        .await
        .map_err(map_identity_error)?;

    Ok((StatusCode::OK, Json(auth)))
}

/// Rotate a valid refresh token and issue a new access + refresh token pair.
#[utoipa::path(
    post,
    path = "/api/v1/refresh",
    tag = "Identity Context",
    request_body = RefreshTokenRequest,
    responses(
        (status = 200, description = "Token pair rotated", body = TokenPairDto),
        (status = 400, description = "Missing refresh token", body = ProblemDetails),
        (status = 401, description = "Invalid or expired refresh token", body = ProblemDetails)
    )
)]
pub async fn refresh_token_handler(
    State(state): State<IdentityHttpState>,
    payload: Result<Json<RefreshTokenRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<TokenPairDto>), ApiError> {
    let payload = extract_json(payload)?;
    let refresh_token = require_non_empty(payload.refresh_token, "MISSING_REFRESH_TOKEN")?;

    let tokens = state
        .refresh_token
        .handle(RefreshTokenCommand { refresh_token })
        .await
        .map_err(map_identity_error)?;

    Ok((StatusCode::OK, Json(tokens)))
}

/// Revoke a refresh token idempotently.
#[utoipa::path(
    post,
    path = "/api/v1/logout",
    tag = "Identity Context",
    request_body = LogoutRequest,
    responses(
        (status = 204, description = "Logged out"),
        (status = 400, description = "Missing refresh token", body = ProblemDetails)
    )
)]
pub async fn logout_user_handler(
    State(state): State<IdentityHttpState>,
    payload: Result<Json<LogoutRequest>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let payload = extract_json(payload)?;
    let refresh_token = require_non_empty(payload.refresh_token, "MISSING_REFRESH_TOKEN")?;

    state
        .logout_user
        .handle(LogoutUserCommand { refresh_token })
        .await
        .map_err(map_identity_error)?;

    Ok(StatusCode::NO_CONTENT)
}

/// Get the currently authenticated user's profile.
#[utoipa::path(
    get,
    path = "/api/v1/me",
    tag = "Identity Context",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Current authenticated user", body = UserDto),
        (status = 401, description = "Unauthorized or invalid token", body = ProblemDetails)
    )
)]
pub async fn me_handler(
    State(state): State<IdentityHttpState>,
    auth: AuthenticatedUser,
) -> Result<(StatusCode, Json<UserDto>), ApiError> {
    let user = state
        .get_current_user
        .handle(GetCurrentUserQuery {
            user_id: UserId::from(auth.user_id),
        })
        .await
        .map_err(map_identity_error)?;

    Ok((StatusCode::OK, Json(user)))
}

#[cfg(test)]
mod tests {
    use super::require_non_empty;

    #[test]
    fn require_non_empty_trims_surrounding_whitespace() {
        let out = require_non_empty(Some("  abc123  ".to_string()), "MISSING_REFRESH_TOKEN")
            .expect("expected trimmed token");
        assert_eq!(out, "abc123");
    }
}
