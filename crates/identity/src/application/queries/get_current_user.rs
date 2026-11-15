use crate::{
    application::{dto::UserDto, errors::IdentityApplicationError},
    domain::{repositories::UserRepository, value_objects::UserId},
};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct GetCurrentUserQuery {
    pub user_id: UserId,
}

#[derive(Clone)]
pub struct GetCurrentUserHandler {
    user_repository: Arc<dyn UserRepository>,
}

impl GetCurrentUserHandler {
    pub fn new(user_repository: Arc<dyn UserRepository>) -> Self {
        Self { user_repository }
    }

    pub async fn handle(
        &self,
        query: GetCurrentUserQuery,
    ) -> Result<UserDto, IdentityApplicationError> {
        let user = self
            .user_repository
            .find_by_id(query.user_id)
            .await?
            .ok_or(IdentityApplicationError::InvalidToken)?;

        if !user.is_active() {
            return Err(IdentityApplicationError::InvalidToken);
        }

        Ok(UserDto::from(&user))
    }
}
