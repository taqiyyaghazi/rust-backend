use crate::{config::AppConfig, openapi::ApiDoc};
use axum::{routing::get, Json, Router};
use identity::{
    domain::repositories::UserRepository,
    infrastructure::PostgresUserRepository,
    presentation::{identity_router, IdentityHttpState},
};
use serde_json::{json, Value};
use shared_kernel::{
    application::EventBus,
    infrastructure::{AuditLogEventSubscriber, InMemoryEventBus},
};
use sqlx::postgres::PgPoolOptions;
use std::{sync::Arc, time::Duration};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
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
    let event_bus = Arc::new(InMemoryEventBus::new());
    event_bus
        .register_subscriber(Arc::new(AuditLogEventSubscriber))
        .await;
    let event_bus_port: Arc<dyn EventBus> = event_bus;

    info!("Connecting to PostgreSQL database...");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await?;

    let user_repository: Arc<dyn UserRepository> = Arc::new(PostgresUserRepository::new(pool));

    let identity_state = IdentityHttpState::new(user_repository, event_bus_port);

    let app = Router::new()
        .route("/health", get(health_check_handler))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(identity_router(identity_state))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    Ok(app)
}
