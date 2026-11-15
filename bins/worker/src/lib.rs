mod openapi;

use axum::{
    extract::{DefaultBodyLimit, State},
    http::{header, HeaderValue, Method, StatusCode},
    routing::get,
    Extension, Json, Router,
};
use identity::{
    domain::repositories::{RefreshTokenRepository, UserRepository},
    infrastructure::{D1RefreshTokenRepository, D1UserRepository},
    presentation::{identity_router, IdentityHttpState},
};
use openapi::ApiDoc;
use serde::Deserialize;
use serde_json::{json, Value};
use shared_kernel::presentation::{ApiError, JwtAuthConfig};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_service::Service;
use utoipa::OpenApi;
use worker::{event, send::SendFuture, Context, D1Database, Env, HttpRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerSettings {
    pub jwt_secret: String,
    pub jwt_access_ttl_secs: u64,
    pub refresh_token_ttl_days: i64,
    pub cors_allowed_origins: Vec<String>,
}

pub fn parse_worker_settings(
    jwt_secret: &str,
    access_ttl_raw: Option<&str>,
    refresh_ttl_raw: Option<&str>,
    cors_origins_raw: Option<&str>,
) -> Result<WorkerSettings, String> {
    if jwt_secret.trim().len() < 32 {
        return Err("JWT_SECRET must be set and at least 32 characters long".to_string());
    }

    let jwt_access_ttl_secs = match access_ttl_raw {
        Some(raw) if !raw.trim().is_empty() => raw
            .trim()
            .parse::<u64>()
            .map_err(|_| "JWT_ACCESS_TTL_SECS must be a positive integer".to_string())?,
        _ => 900,
    };

    let refresh_token_ttl_days = match refresh_ttl_raw {
        Some(raw) if !raw.trim().is_empty() => raw
            .trim()
            .parse::<i64>()
            .map_err(|_| "REFRESH_TOKEN_TTL_DAYS must be a positive integer".to_string())?,
        _ => 30,
    };
    if refresh_token_ttl_days <= 0 {
        return Err("REFRESH_TOKEN_TTL_DAYS must be greater than 0".to_string());
    }

    let cors_allowed_origins = cors_origins_raw
        .unwrap_or("http://localhost:3000")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect();

    Ok(WorkerSettings {
        jwt_secret: jwt_secret.to_string(),
        jwt_access_ttl_secs,
        refresh_token_ttl_days,
        cors_allowed_origins,
    })
}

#[derive(Clone)]
struct SendD1(Arc<D1Database>);

// SAFETY: Cloudflare Workers (`wasm32-unknown-unknown`) runs in a single-threaded isolate.
unsafe impl Send for SendD1 {}
// SAFETY: Cloudflare Workers (`wasm32-unknown-unknown`) runs in a single-threaded isolate.
unsafe impl Sync for SendD1 {}

#[derive(Debug, Deserialize)]
struct HealthRow {
    #[allow(dead_code)]
    ok: i64,
}

async fn health_check_handler(State(db): State<SendD1>) -> (StatusCode, Json<Value>) {
    let check = SendFuture::new(async move {
        let stmt = db.0.prepare("SELECT 1 as ok");
        stmt.first::<HealthRow>(None).await
    })
    .await;

    let (status_code, status) = match check {
        Ok(Some(_)) => (StatusCode::OK, "ok"),
        Ok(None) | Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    };

    (
        status_code,
        Json(json!({
            "status": status,
            "architecture": "ddd-modular-monolith",
            "bounded_contexts": ["identity"],
            "storage": "cloudflare-d1"
        })),
    )
}

async fn openapi_json_handler() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

fn read_optional_var(env: &Env, key: &str) -> Option<String> {
    env.var(key).ok().map(|v| v.to_string())
}

fn build_worker_router(env: &Env) -> worker::Result<Router> {
    let jwt_secret = env
        .secret("JWT_SECRET")
        .or_else(|_| env.var("JWT_SECRET"))
        .map(|v| v.to_string())
        .map_err(|_| worker::Error::RustError("Missing JWT_SECRET binding".to_string()))?;

    let access_ttl = read_optional_var(env, "JWT_ACCESS_TTL_SECS");
    let refresh_ttl = read_optional_var(env, "REFRESH_TOKEN_TTL_DAYS");
    let cors_origins = read_optional_var(env, "CORS_ALLOWED_ORIGINS");

    let settings = parse_worker_settings(
        &jwt_secret,
        access_ttl.as_deref(),
        refresh_ttl.as_deref(),
        cors_origins.as_deref(),
    )
    .map_err(worker::Error::RustError)?;

    let db = Arc::new(env.d1("DB")?);

    let user_repository: Arc<dyn UserRepository> = Arc::new(D1UserRepository::new(db.clone()));
    let refresh_token_repository: Arc<dyn RefreshTokenRepository> =
        Arc::new(D1RefreshTokenRepository::new(db.clone()));
    let jwt_config = JwtAuthConfig::new(settings.jwt_secret, settings.jwt_access_ttl_secs);

    let identity_state = IdentityHttpState::new(
        user_repository,
        refresh_token_repository,
        jwt_config.clone(),
        settings.refresh_token_ttl_days,
    );

    let allowed_origins: Vec<HeaderValue> = settings
        .cors_allowed_origins
        .iter()
        .map(|origin| HeaderValue::from_str(origin))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| worker::Error::RustError(format!("Invalid CORS origin: {e}")))?;

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    let app = Router::new()
        .route("/health", get(health_check_handler).with_state(SendD1(db)))
        .route("/api-docs/openapi.json", get(openapi_json_handler))
        .merge(identity_router(identity_state))
        .fallback(|| async { ApiError::not_found() })
        .method_not_allowed_fallback(|| async { ApiError::method_not_allowed() })
        .layer(Extension(jwt_config))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(cors);

    Ok(app)
}

#[event(fetch)]
pub async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> worker::Result<http::Response<axum::body::Body>> {
    let mut router = build_worker_router(&env)?;
    router
        .call(req)
        .await
        .map_err(|e| worker::Error::RustError(format!("Router error: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_jwt_secret_shorter_than_32_chars() {
        let err = parse_worker_settings("short-secret", None, None, None).unwrap_err();
        assert!(err.contains("at least 32 characters"));
    }

    #[test]
    fn parses_defaults_and_comma_separated_cors_origins() {
        let settings = parse_worker_settings(
            "0123456789abcdef0123456789abcdef",
            None,
            None,
            Some("https://staging.example.com, https://app.example.com "),
        )
        .expect("valid settings");

        assert_eq!(settings.jwt_access_ttl_secs, 900);
        assert_eq!(settings.refresh_token_ttl_days, 30);
        assert_eq!(
            settings.cors_allowed_origins,
            vec![
                "https://staging.example.com".to_string(),
                "https://app.example.com".to_string()
            ]
        );
    }
}
