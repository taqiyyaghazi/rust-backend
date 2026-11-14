use crate::{application::commands::RegisterUserHandler, domain::repositories::UserRepository};
use std::sync::Arc;

/// Presentation state holding the `RegisterUserHandler` use case.
#[derive(Clone)]
pub struct IdentityHttpState {
    pub register_user: RegisterUserHandler,
}

impl IdentityHttpState {
    pub fn new(user_repository: Arc<dyn UserRepository>) -> Self {
        Self {
            register_user: RegisterUserHandler::new(user_repository),
        }
    }
}
