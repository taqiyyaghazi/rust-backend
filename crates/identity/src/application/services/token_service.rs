use crate::{
    application::{dto::TokenPairDto, errors::IdentityApplicationError},
    domain::{
        entities::RefreshToken,
        repositories::RefreshTokenRepository,
        value_objects::UserId,
    },
};
use argon2::password_hash::rand_core::{OsRng, RngCore};
use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use shared_kernel::presentation::{issue_access_token, JwtAuthConfig};
use std::sync::Arc;

#[derive(Clone)]
pub struct TokenService {
    refresh_token_repository: Arc<dyn RefreshTokenRepository>,
    jwt_config: JwtAuthConfig,
    refresh_ttl_days: i64,
}

impl TokenService {
    pub fn new(
        refresh_token_repository: Arc<dyn RefreshTokenRepository>,
        jwt_config: JwtAuthConfig,
        refresh_ttl_days: i64,
    ) -> Self {
        Self {
            refresh_token_repository,
            jwt_config,
            refresh_ttl_days,
        }
    }

    pub fn hash_refresh_token(raw: &str) -> String {
        let digest = Sha256::digest(raw.as_bytes());
        hex::encode(digest)
    }

    pub async fn issue_token_pair(
        &self,
        user_id: UserId,
    ) -> Result<TokenPairDto, IdentityApplicationError> {
        let mut raw_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut raw_bytes);
        let refresh_token = hex::encode(raw_bytes);
        let token_hash = Self::hash_refresh_token(&refresh_token);
        let expires_at = Utc::now() + Duration::days(self.refresh_ttl_days);

        let entity = RefreshToken::new(user_id, token_hash, expires_at);
        self.refresh_token_repository.save(&entity).await?;

        let access_token = issue_access_token(user_id.as_uuid(), &self.jwt_config)
            .map_err(|_| IdentityApplicationError::TokenGeneration)?;

        Ok(TokenPairDto {
            access_token,
            refresh_token,
            expires_in: self.jwt_config.access_ttl_secs,
        })
    }

    pub async fn rotate_refresh_token(
        &self,
        raw_token: &str,
    ) -> Result<Option<UserId>, IdentityApplicationError> {
        let token_hash = Self::hash_refresh_token(raw_token);
        let user_id = self
            .refresh_token_repository
            .revoke_and_get_user_id(&token_hash, Utc::now())
            .await?;
        Ok(user_id)
    }

    pub async fn revoke_refresh_token(
        &self,
        raw_token: &str,
    ) -> Result<(), IdentityApplicationError> {
        let token_hash = Self::hash_refresh_token(raw_token);
        self.refresh_token_repository
            .revoke(&token_hash, Utc::now())
            .await?;
        Ok(())
    }
}
