use crate::{
    application::{dto::UserDto, errors::IdentityApplicationError},
    domain::{
        entities::User,
        repositories::UserRepository,
        value_objects::{Email, FullName},
    },
};
use shared_kernel::application::EventBus;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct RegisterUserCommand {
    pub email: String,
    pub full_name: String,
}

/// Use-Case Command Handler for registering a new User Aggregate.
#[derive(Clone)]
pub struct RegisterUserHandler {
    user_repository: Arc<dyn UserRepository>,
    event_bus: Arc<dyn EventBus>,
}

impl RegisterUserHandler {
    pub fn new(user_repository: Arc<dyn UserRepository>, event_bus: Arc<dyn EventBus>) -> Self {
        Self {
            user_repository,
            event_bus,
        }
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

        let mut user = User::register(email, full_name);
        self.user_repository.save(&user).await?;

        let events = user.pull_domain_events();
        self.event_bus.publish_all(events).await;

        Ok(UserDto::from(&user))
    }
}
