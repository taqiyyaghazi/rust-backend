use crate::{
    application::{
        dto::TokenPairDto, errors::IdentityApplicationError, services::TokenService,
    },
    domain::repositories::UserRepository,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct RefreshTokenCommand {
    pub refresh_token: String,
}

#[derive(Clone)]
pub struct RefreshTokenHandler {
    user_repository: Arc<dyn UserRepository>,
    token_service: Arc<TokenService>,
}

impl RefreshTokenHandler {
    pub fn new(
        user_repository: Arc<dyn UserRepository>,
        token_service: Arc<TokenService>,
    ) -> Self {
        Self {
            user_repository,
            token_service,
        }
    }

    pub async fn handle(
        &self,
        cmd: RefreshTokenCommand,
    ) -> Result<TokenPairDto, IdentityApplicationError> {
        let user_id = self
            .token_service
            .rotate_refresh_token(&cmd.refresh_token)
            .await?
            .ok_or(IdentityApplicationError::InvalidRefreshToken)?;

        let user = self
            .user_repository
            .find_by_id(user_id)
            .await?
            .ok_or(IdentityApplicationError::InvalidRefreshToken)?;

        if !user.is_active() {
            return Err(IdentityApplicationError::InvalidRefreshToken);
        }

        self.token_service.issue_token_pair(user.id()).await
    }
}
