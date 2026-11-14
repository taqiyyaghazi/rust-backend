use crate::{
    application::{dto::UserDto, errors::IdentityApplicationError},
    domain::{
        entities::User,
        repositories::{RepositoryError, UserRepository},
        value_objects::{Email, FullName},
    },
};
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Clone)]
pub struct RegisterUserCommand {
    pub email: String,
    pub full_name: String,
}

/// Use-Case Command Handler for registering a new User Aggregate.
#[derive(Clone)]
pub struct RegisterUserHandler {
    user_repository: Arc<dyn UserRepository>,
}

impl RegisterUserHandler {
    pub fn new(user_repository: Arc<dyn UserRepository>) -> Self {
        Self { user_repository }
    }

    pub async fn handle(
        &self,
        cmd: RegisterUserCommand,
    ) -> Result<UserDto, IdentityApplicationError> {
        let email = Email::parse(cmd.email)?;
        let full_name = FullName::parse(cmd.full_name)?;

        if self.user_repository.find_by_email(&email).await?.is_some() {
            return Err(IdentityApplicationError::EmailAlreadyExists(
                email.to_string(),
            ));
        }

        let user = User::register(email, full_name);
        if let Err(err) = self.user_repository.save(&user).await {
            return match err {
                RepositoryError::Conflict => Err(IdentityApplicationError::EmailAlreadyExists(
                    user.email().to_string(),
                )),
                other => Err(other.into()),
            };
        }

        info!(user_id = %user.id(), email = %user.email(), "User registered");

        Ok(UserDto::from(&user))
    }
}
