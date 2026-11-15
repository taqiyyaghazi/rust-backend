use crate::{
    application::{
        dto::{AuthResponseDto, UserDto},
        errors::IdentityApplicationError,
        services::{password_hasher, TokenService},
    },
    domain::{
        entities::User,
        errors::IdentityDomainError,
        repositories::{RepositoryError, UserRepository},
        value_objects::{Email, FullName},
    },
};
use std::sync::Arc;
use tracing::info;

#[derive(Clone)]
pub struct RegisterUserCommand {
    pub email: String,
    pub full_name: String,
    pub password: String,
}

/// Use-Case Command Handler for registering a new User Aggregate.
#[derive(Clone)]
pub struct RegisterUserHandler {
    user_repository: Arc<dyn UserRepository>,
    token_service: Arc<TokenService>,
    hash_semaphore: Arc<tokio::sync::Semaphore>,
}

impl RegisterUserHandler {
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
        cmd: RegisterUserCommand,
    ) -> Result<AuthResponseDto, IdentityApplicationError> {
        let email = Email::parse(cmd.email)?;
        let full_name = FullName::parse(cmd.full_name)?;
        if !(8..=128).contains(&cmd.password.chars().count()) {
            return Err(IdentityDomainError::InvalidPassword.into());
        }

        if self.user_repository.exists_by_email(&email).await? {
            return Err(IdentityApplicationError::EmailAlreadyExists(
                email.to_string(),
            ));
        }

        let password_hash =
            password_hasher::hash_password(cmd.password, &self.hash_semaphore).await?;

        let user = User::register(email, full_name, password_hash);
        if let Err(err) = self.user_repository.save(&user).await {
            return match err {
                RepositoryError::Conflict => Err(IdentityApplicationError::EmailAlreadyExists(
                    user.email().to_string(),
                )),
                other => Err(other.into()),
            };
        }

        info!(user_id = %user.id(), "User registered");

        let tokens = self.token_service.issue_token_pair(user.id()).await?;

        Ok(AuthResponseDto {
            user: UserDto::from(&user),
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            expires_in: tokens.expires_in,
        })
    }
}
