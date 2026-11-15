use crate::domain::{
    entities::User,
    value_objects::{Email, UserId},
};
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Unique constraint conflict")]
    Conflict,

    #[error("Database/storage error: {0}")]
    Storage(String),
}

/// Repository Port owned by the `identity` Domain Layer.
/// Implemented by `PostgresUserRepository` in `infrastructure`.
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn save(&self, user: &User) -> Result<(), RepositoryError>;
    async fn exists_by_email(&self, email: &Email) -> Result<bool, RepositoryError>;
    async fn find_by_email(&self, email: &Email) -> Result<Option<User>, RepositoryError>;
    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError>;
}
