use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub cors_allowed_origins: Vec<String>,
    pub enable_swagger_ui: bool,
    pub jwt_secret: String,
    pub jwt_access_ttl_secs: u64,
    pub refresh_token_ttl_days: i64,
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

        let enable_swagger_ui = env::var("ENABLE_SWAGGER_UI")
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);

        let jwt_secret = env::var("JWT_SECRET").unwrap_or_default();
        let jwt_access_ttl_secs = env::var("JWT_ACCESS_TTL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(900);
        let refresh_token_ttl_days = env::var("REFRESH_TOKEN_TTL_DAYS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(7);

        Self {
            host,
            port,
            database_url,
            cors_allowed_origins,
            enable_swagger_ui,
            jwt_secret,
            jwt_access_ttl_secs,
            refresh_token_ttl_days,
        }
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.jwt_secret.trim().len() < 32 {
            anyhow::bail!("JWT_SECRET must be set and at least 32 characters long");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    fn sample_config(secret: &str) -> AppConfig {
        AppConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            database_url: "postgres://localhost/db".to_string(),
            cors_allowed_origins: vec![],
            enable_swagger_ui: false,
            jwt_secret: secret.to_string(),
            jwt_access_ttl_secs: 900,
            refresh_token_ttl_days: 7,
        }
    }

    #[test]
    fn validate_rejects_empty_or_short_jwt_secret() {
        assert!(sample_config("").validate().is_err());
        assert!(sample_config("   ").validate().is_err());
        assert!(sample_config("short-secret").validate().is_err());
    }

    #[test]
    fn validate_accepts_32_byte_jwt_secret() {
        assert!(sample_config("0123456789abcdef0123456789abcdef").validate().is_ok());
    }
}
