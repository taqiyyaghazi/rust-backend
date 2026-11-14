use crate::{config::AppConfig, openapi::ApiDoc};
use axum::{
    http::{header, HeaderValue, Method},
    routing::get,
    Json, Router,
};
use identity::{
    domain::repositories::UserRepository,
    infrastructure::PostgresUserRepository,
    presentation::{identity_router, IdentityHttpState},
};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use std::{sync::Arc, time::Duration};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

async fn health_check_handler() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "architecture": "ddd-modular-monolith",
        "bounded_contexts": ["identity"],
        "storage": "postgres"
    }))
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

    let user_repository: Arc<dyn UserRepository> = Arc::new(PostgresUserRepository::new(pool));

    let identity_state = IdentityHttpState::new(user_repository);

    let allowed_origins: Vec<HeaderValue> = config
        .cors_allowed_origins
        .iter()
        .map(|origin| HeaderValue::from_str(origin))
        .collect::<Result<Vec<_>, _>>()?;

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    let app = Router::new()
        .route("/health", get(health_check_handler))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(identity_router(identity_state))
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    Ok(app)
}
