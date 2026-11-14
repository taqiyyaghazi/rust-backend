use crate::presentation::{http::handlers::register_user_handler, state::IdentityHttpState};
use axum::{routing::post, Router};

/// Builds the self-contained Axum `Router` for the `identity` Bounded Context.
pub fn identity_router(state: IdentityHttpState) -> Router {
    Router::new()
        .route("/api/v1/register", post(register_user_handler))
        .with_state(state)
}
