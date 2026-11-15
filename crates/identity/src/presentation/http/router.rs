use crate::presentation::{
    http::handlers::{
        login_user_handler, logout_user_handler, me_handler, refresh_token_handler,
        register_user_handler,
    },
    state::IdentityHttpState,
};
use axum::{
    routing::{get, post},
    Router,
};

/// Builds the self-contained Axum `Router` for the `identity` Bounded Context.
pub fn identity_router(state: IdentityHttpState) -> Router {
    Router::new()
        .route("/api/v1/register", post(register_user_handler))
        .route("/api/v1/login", post(login_user_handler))
        .route("/api/v1/refresh", post(refresh_token_handler))
        .route("/api/v1/logout", post(logout_user_handler))
        .route("/api/v1/me", get(me_handler))
        .with_state(state)
}
