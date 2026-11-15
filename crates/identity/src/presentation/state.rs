use crate::{
    application::{
        commands::{LoginUserHandler, LogoutUserHandler, RefreshTokenHandler, RegisterUserHandler},
        queries::GetCurrentUserHandler,
        services::TokenService,
    },
    domain::repositories::{RefreshTokenRepository, UserRepository},
};
use shared_kernel::presentation::JwtAuthConfig;
use std::sync::Arc;

/// Presentation state holding the Identity Bounded Context use cases.
#[derive(Clone)]
pub struct IdentityHttpState {
    pub register_user: RegisterUserHandler,
    pub login_user: LoginUserHandler,
    pub refresh_token: RefreshTokenHandler,
    pub logout_user: LogoutUserHandler,
    pub get_current_user: GetCurrentUserHandler,
}

impl IdentityHttpState {
    pub fn new(
        user_repository: Arc<dyn UserRepository>,
        refresh_token_repository: Arc<dyn RefreshTokenRepository>,
        jwt_config: JwtAuthConfig,
        refresh_ttl_days: i64,
    ) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let max_concurrent_hashes = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        #[cfg(target_arch = "wasm32")]
        let max_concurrent_hashes = 1;
        let hash_semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent_hashes));
        let token_service = Arc::new(TokenService::new(
            refresh_token_repository,
            jwt_config,
            refresh_ttl_days,
        ));

        Self {
            register_user: RegisterUserHandler::new(
                user_repository.clone(),
                token_service.clone(),
                hash_semaphore.clone(),
            ),
            login_user: LoginUserHandler::new(
                user_repository.clone(),
                token_service.clone(),
                hash_semaphore,
            ),
            refresh_token: RefreshTokenHandler::new(user_repository.clone(), token_service.clone()),
            logout_user: LogoutUserHandler::new(token_service),
            get_current_user: GetCurrentUserHandler::new(user_repository),
        }
    }
}
