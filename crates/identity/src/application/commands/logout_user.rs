use crate::application::{errors::IdentityApplicationError, services::TokenService};
use std::sync::Arc;

#[derive(Clone)]
pub struct LogoutUserCommand {
    pub refresh_token: String,
}

#[derive(Clone)]
pub struct LogoutUserHandler {
    token_service: Arc<TokenService>,
}

impl LogoutUserHandler {
    pub fn new(token_service: Arc<TokenService>) -> Self {
        Self { token_service }
    }

    pub async fn handle(&self, cmd: LogoutUserCommand) -> Result<(), IdentityApplicationError> {
        self.token_service
            .revoke_refresh_token(&cmd.refresh_token)
            .await
    }
}
