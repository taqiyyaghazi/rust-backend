use crate::{
    application::{dto::UserDto, errors::IdentityApplicationError},
    domain::{
        entities::User,
        errors::IdentityDomainError,
        repositories::{RepositoryError, UserRepository},
        value_objects::{Email, FullName},
    },
};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
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
    hash_semaphore: Arc<tokio::sync::Semaphore>,
}

impl RegisterUserHandler {
    pub fn new(user_repository: Arc<dyn UserRepository>) -> Self {
        let max_concurrent_hashes = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Self {
            user_repository,
            hash_semaphore: Arc::new(tokio::sync::Semaphore::new(max_concurrent_hashes)),
        }
    }

    pub async fn handle(
        &self,
        cmd: RegisterUserCommand,
    ) -> Result<UserDto, IdentityApplicationError> {
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

        let _permit = self
            .hash_semaphore
            .acquire()
            .await
            .map_err(|_| IdentityApplicationError::PasswordHashing)?;
        let raw_password = cmd.password;
        let password_hash = tokio::task::spawn_blocking(move || {
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(raw_password.as_bytes(), &salt)
                .map(|h| h.to_string())
        })
        .await
        .map_err(|_| IdentityApplicationError::PasswordHashing)?
        .map_err(|_| IdentityApplicationError::PasswordHashing)?;
        drop(_permit);

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

        Ok(UserDto::from(&user))
    }
}
