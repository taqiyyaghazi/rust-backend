use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Minimal JSON error response body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetails {
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

    pub fn conflict(error_code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            error_code,
            title: "Domain Conflict / Rule Violation",
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
            error_code: self.error_code.to_string(),
        };

        (self.status, Json(body)).into_response()
    }
}
