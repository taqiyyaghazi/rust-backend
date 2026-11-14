use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// RFC 7807 / RFC 9457 Problem Details response body for consistent HTTP error reporting.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProblemDetails {
    /// URI reference that identifies the problem type.
    #[serde(rename = "type")]
    pub problem_type: String,
    /// Short, human-readable summary of the problem type.
    pub title: String,
    /// HTTP status code.
    pub status: u16,
    /// Human-readable explanation specific to this occurrence of the problem.
    pub detail: String,
    /// Machine-readable domain/application error code.
    pub error_code: String,
}

/// Unified HTTP API Error wrapper used by presentation handlers across bounded contexts.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub error_code: &'static str,
    pub title: &'static str,
    pub detail: String,
}

impl ApiError {
    pub fn bad_request(error_code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            error_code,
            title: "Bad Request",
            detail: detail.into(),
        }
    }

    pub fn not_found(error_code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            error_code,
            title: "Resource Not Found",
            detail: detail.into(),
        }
    }

    pub fn conflict(error_code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            error_code,
            title: "Domain Conflict / Rule Violation",
            detail: detail.into(),
        }
    }

    pub fn unprocessable(error_code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            error_code,
            title: "Unprocessable Domain Rule",
            detail: detail.into(),
        }
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error_code: "INTERNAL_SERVER_ERROR",
            title: "Internal Server Error",
            detail: detail.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ProblemDetails {
            problem_type: format!("urn:ddd-backend:error:{}", self.error_code.to_lowercase()),
            title: self.title.to_string(),
            status: self.status.as_u16(),
            detail: self.detail,
            error_code: self.error_code.to_string(),
        };

        (self.status, Json(body)).into_response()
    }
}
