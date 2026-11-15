use crate::domain::{entities::RefreshToken, repositories::RepositoryError, value_objects::UserId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[async_trait]
pub trait RefreshTokenRepository: Send + Sync {
    async fn save(&self, token: &RefreshToken) -> Result<(), RepositoryError>;
    async fn revoke_and_get_user_id(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<UserId>, RepositoryError>;
    async fn revoke(&self, token_hash: &str, now: DateTime<Utc>) -> Result<(), RepositoryError>;
}
