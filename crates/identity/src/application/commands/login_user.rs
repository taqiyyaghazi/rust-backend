use crate::{
    application::{
        dto::{AuthResponseDto, UserDto},
        errors::IdentityApplicationError,
        services::TokenService,
    },
    domain::{
        errors::IdentityDomainError,
        repositories::UserRepository,
        value_objects::Email,
    },
};
use argon2::{
    password_hash::{PasswordHash, PasswordVerifier},
    Argon2,
};
use std::sync::Arc;

// Valid Argon2id PHC hash used when an email is not found so timing matches a real verification.
const DUMMY_PASSWORD_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

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
        let target_hash = user_opt
            .as_ref()
            .map(|u| u.password_hash().to_string())
            .unwrap_or_else(|| DUMMY_PASSWORD_HASH.to_string());

        let _permit = self
            .hash_semaphore
            .acquire()
            .await
            .map_err(|_| IdentityApplicationError::PasswordHashing)?;
        let raw_password = cmd.password;
        let is_valid = tokio::task::spawn_blocking(move || {
            let parsed = PasswordHash::new(&target_hash).ok()?;
            Some(
                Argon2::default()
                    .verify_password(raw_password.as_bytes(), &parsed)
                    .is_ok(),
            )
        })
        .await
        .map_err(|_| IdentityApplicationError::PasswordHashing)?
        .unwrap_or(false);
        drop(_permit);

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
