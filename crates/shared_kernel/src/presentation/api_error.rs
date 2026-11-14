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
}

impl ApiError {
    pub fn bad_request(error_code: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            error_code,
        }
    }

    pub fn conflict(error_code: &'static str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            error_code,
        }
    }

    pub fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            error_code: "NOT_FOUND",
        }
    }

    pub fn method_not_allowed() -> Self {
        Self {
            status: StatusCode::METHOD_NOT_ALLOWED,
            error_code: "METHOD_NOT_ALLOWED",
        }
    }

    pub fn payload_too_large() -> Self {
        Self {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            error_code: "PAYLOAD_TOO_LARGE",
        }
    }

    pub fn internal() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error_code: "INTERNAL_SERVER_ERROR",
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
