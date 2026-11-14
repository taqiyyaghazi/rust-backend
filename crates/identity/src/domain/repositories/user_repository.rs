use crate::domain::{entities::User, value_objects::Email};
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Database/storage error: {0}")]
    Storage(String),

    #[error("Data corruption when reconstituting domain entity: {0}")]
    DataCorruption(String),
}

/// Repository Port owned by the `identity` Domain Layer.
/// Implemented by `PostgresUserRepository` and `InMemoryUserRepository` in `infrastructure`.
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn save(&self, user: &User) -> Result<(), RepositoryError>;
    async fn find_by_email(&self, email: &Email) -> Result<Option<User>, RepositoryError>;
}
