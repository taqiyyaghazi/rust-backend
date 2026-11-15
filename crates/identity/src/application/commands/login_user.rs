use crate::{
    application::{
        dto::{AuthResponseDto, UserDto},
        errors::IdentityApplicationError,
        services::{password_hasher, TokenService},
    },
    domain::{
        errors::IdentityDomainError,
        repositories::UserRepository,
        value_objects::Email,
    },
};
use std::sync::Arc;

#[derive(Clone)]
pub struct LoginUserCommand {
    pub email: String,
    pub password: String,
}

#[derive(Clone)]
pub struct LoginUserHandler {
    user_repository: Arc<dyn UserRepository>,
    token_service: Arc<TokenService>,
    hash_semaphore: Arc<tokio::sync::Semaphore>,
}

impl LoginUserHandler {
    pub fn new(
        user_repository: Arc<dyn UserRepository>,
        token_service: Arc<TokenService>,
        hash_semaphore: Arc<tokio::sync::Semaphore>,
    ) -> Self {
        Self {
            user_repository,
            token_service,
            hash_semaphore,
        }
    }

    pub async fn handle(
        &self,
        cmd: LoginUserCommand,
    ) -> Result<AuthResponseDto, IdentityApplicationError> {
        let email = Email::parse(cmd.email)?;
        if !(8..=128).contains(&cmd.password.chars().count()) {
            return Err(IdentityDomainError::InvalidPassword.into());
        }

        let user_opt = self.user_repository.find_by_email(&email).await?;
        let target_hash = user_opt.as_ref().map(|u| u.password_hash().to_string());
        let is_valid =
            password_hasher::verify_password(cmd.password, target_hash, &self.hash_semaphore)
                .await?;

        let Some(user) = user_opt else {
            return Err(IdentityApplicationError::InvalidCredentials);
        };

        if !is_valid || !user.is_active() {
            return Err(IdentityApplicationError::InvalidCredentials);
        }

        let tokens = self.token_service.issue_token_pair(user.id()).await?;

        Ok(AuthResponseDto {
            user: UserDto::from(&user),
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            expires_in: tokens.expires_in,
        })
    }
}
