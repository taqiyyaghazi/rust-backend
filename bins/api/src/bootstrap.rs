use crate::{config::AppConfig, openapi::ApiDoc};
use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::get,
    Extension, Json, Router,
};
use identity::{
    domain::repositories::{RefreshTokenRepository, UserRepository},
    infrastructure::{PostgresRefreshTokenRepository, PostgresUserRepository},
    presentation::{identity_router, IdentityHttpState},
};
use serde_json::{json, Value};
use shared_kernel::presentation::{ApiError, JwtAuthConfig};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

async fn log_request(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let endpoint = req.uri().path().to_string();
    let start = Instant::now();

    let res = next.run(req).await;

    info!(
        method = %method,
        endpoint = %endpoint,
        status = res.status().as_u16(),
        latency_ms = start.elapsed().as_millis() as u64,
        "HTTP request"
    );

    res
}

async fn health_check_handler(State(pool): State<PgPool>) -> (StatusCode, Json<Value>) {
    let (status_code, status) = match sqlx::query("SELECT 1").execute(&pool).await {
        Ok(_) => (StatusCode::OK, "ok"),
        Err(err) => {
            tracing::error!(error = %err, "Database health check failed");
            (StatusCode::SERVICE_UNAVAILABLE, "unavailable")
        }
    };

    (
        status_code,
        Json(json!({
            "status": status,
            "architecture": "ddd-modular-monolith",
            "bounded_contexts": ["identity"],
            "storage": "postgres"
        })),
    )
}

/// Builds the complete Modular Monolith application router with PostgreSQL storage.
///
/// NOTE: Database migrations are NOT executed here. Migrations must be run separately
/// via `cargo run --bin migrate` (or `./scripts/migrate.sh`) prior to starting the API server.
pub async fn build_app(config: &AppConfig) -> anyhow::Result<Router> {
    info!("Connecting to PostgreSQL database...");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await?;

    let user_repository: Arc<dyn UserRepository> = Arc::new(PostgresUserRepository::new(pool.clone()));
    let refresh_token_repository: Arc<dyn RefreshTokenRepository> =
        Arc::new(PostgresRefreshTokenRepository::new(pool.clone()));
    let jwt_config = JwtAuthConfig::new(config.jwt_secret.clone(), config.jwt_access_ttl_secs);

    let identity_state = IdentityHttpState::new(
        user_repository,
        refresh_token_repository,
        jwt_config.clone(),
        config.refresh_token_ttl_days,
    );

    let allowed_origins: Vec<HeaderValue> = config
        .cors_allowed_origins
        .iter()
        .map(|origin| HeaderValue::from_str(origin))
        .collect::<Result<Vec<_>, _>>()?;

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    let mut app = Router::new()
        .route("/health", get(health_check_handler).with_state(pool))
        .merge(identity_router(identity_state));

    if config.enable_swagger_ui {
        app = app.merge(
            SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()),
        );
    }

    let app = app
        .fallback(|| async { ApiError::not_found() })
        .method_not_allowed_fallback(|| async { ApiError::method_not_allowed() })
        .layer(Extension(jwt_config))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(cors)
        .layer(middleware::from_fn(log_request));

    Ok(app)
}
