use crate::presentation::ApiError;
use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct JwtAuthConfig {
    pub secret: String,
    pub access_ttl_secs: u64,
}

impl JwtAuthConfig {
    pub fn new(secret: impl Into<String>, access_ttl_secs: u64) -> Self {
        Self {
            secret: secret.into(),
            access_ttl_secs,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub iat: i64,
    pub exp: i64,
}

pub fn issue_access_token(
    user_id: Uuid,
    config: &JwtAuthConfig,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now().timestamp();
    let claims = JwtClaims {
        sub: user_id.to_string(),
        iat: now,
        exp: now + config.access_ttl_secs as i64,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(config.secret.as_bytes()),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticatedUser {
    pub user_id: Uuid,
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let config = parts
            .extensions
            .get::<JwtAuthConfig>()
            .ok_or_else(ApiError::internal)?;

        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::unauthorized("UNAUTHORIZED"))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| ApiError::unauthorized("UNAUTHORIZED"))?;

        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        validation.leeway = 0;

        let token_data = decode::<JwtClaims>(
            token,
            &DecodingKey::from_secret(config.secret.as_bytes()),
            &validation,
        )
        .map_err(|_| ApiError::unauthorized("INVALID_TOKEN"))?;

        let user_id = Uuid::parse_str(&token_data.claims.sub)
            .map_err(|_| ApiError::unauthorized("INVALID_TOKEN"))?;

        Ok(Self { user_id })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Request, StatusCode};
    use chrono::Utc;
    use jsonwebtoken::{encode, EncodingKey, Header};

    fn test_config() -> JwtAuthConfig {
        JwtAuthConfig::new("test-secret-key-at-least-32-bytes-long", 900)
    }

    async fn extract_with_header(
        auth_header: Option<&str>,
        config: JwtAuthConfig,
    ) -> Result<AuthenticatedUser, ApiError> {
        let mut builder = Request::builder().uri("/");
        if let Some(val) = auth_header {
            builder = builder.header(AUTHORIZATION, val);
        }
        let mut req = builder.body(()).unwrap();
        req.extensions_mut().insert(config);
        let (mut parts, _) = req.into_parts();
        AuthenticatedUser::from_request_parts(&mut parts, &()).await
    }

    #[tokio::test]
    async fn missing_authorization_header_returns_unauthorized() {
        let err = extract_with_header(None, test_config()).await.unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
        assert_eq!(err.error_code, "UNAUTHORIZED");
    }

    #[tokio::test]
    async fn non_bearer_header_returns_unauthorized() {
        let err = extract_with_header(Some("Basic abc123"), test_config())
            .await
            .unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
        assert_eq!(err.error_code, "UNAUTHORIZED");
    }

    #[tokio::test]
    async fn invalid_token_returns_invalid_token() {
        let err = extract_with_header(Some("Bearer not-a-jwt"), test_config())
            .await
            .unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
        assert_eq!(err.error_code, "INVALID_TOKEN");
    }

    #[tokio::test]
    async fn expired_token_returns_invalid_token() {
        let config = test_config();
        let now = Utc::now().timestamp();
        let claims = JwtClaims {
            sub: Uuid::new_v4().to_string(),
            iat: now - 120,
            exp: now - 60,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(config.secret.as_bytes()),
        )
        .unwrap();

        let err = extract_with_header(Some(&format!("Bearer {token}")), config)
            .await
            .unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
        assert_eq!(err.error_code, "INVALID_TOKEN");
    }

    #[tokio::test]
    async fn non_uuid_sub_returns_invalid_token() {
        let config = test_config();
        let now = Utc::now().timestamp();
        let claims = JwtClaims {
            sub: "not-a-uuid".to_string(),
            iat: now,
            exp: now + 900,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(config.secret.as_bytes()),
        )
        .unwrap();

        let err = extract_with_header(Some(&format!("Bearer {token}")), config)
            .await
            .unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
        assert_eq!(err.error_code, "INVALID_TOKEN");
    }

    #[tokio::test]
    async fn valid_bearer_token_extracts_user_id() {
        let config = test_config();
        let user_id = Uuid::new_v4();
        let token = issue_access_token(user_id, &config).unwrap();

        let extracted = extract_with_header(Some(&format!("Bearer {token}")), config)
            .await
            .unwrap();
        assert_eq!(extracted.user_id, user_id);
    }
}
