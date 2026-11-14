use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub cors_allowed_origins: Vec<String>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();

        let host = env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("APP_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(8080);

        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://ddd_user:ddd_password@localhost:5432/ddd_backend".to_string()
        });

        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();

        Self {
            host,
            port,
            database_url,
            cors_allowed_origins,
        }
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
