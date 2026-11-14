use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
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

        Self {
            host,
            port,
            database_url,
        }
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
