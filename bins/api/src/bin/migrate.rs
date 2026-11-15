use api::config::AppConfig;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env();

    info!("Connecting to PostgreSQL for running database migrations...");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&config.database_url)
        .await?;

    info!("Executing SQLx migrations from ./migrations directory...");
    sqlx::migrate!("../../migrations").run(&pool).await?;

    info!("Database migrations applied successfully!");
    Ok(())
}
