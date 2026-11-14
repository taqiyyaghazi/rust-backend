use api::{bootstrap::build_app, config::AppConfig};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "info,api=debug,identity=debug,shared_kernel=debug,tower_http=info".into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env();
    let bind_addr = config.bind_address();

    info!(
        address = %bind_addr,
        "Starting Rust DDD Modular Monolith API Server"
    );

    let app = build_app(&config).await?;
    let listener = TcpListener::bind(&bind_addr).await?;

    info!(
        swagger_ui = %format!("http://{}/swagger-ui", bind_addr),
        health_check = %format!("http://{}/health", bind_addr),
        "Server listening and ready to accept connections"
    );

    axum::serve(listener, app).await?;
    Ok(())
}
