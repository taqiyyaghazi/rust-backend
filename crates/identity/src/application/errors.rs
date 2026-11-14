use crate::domain::{errors::IdentityDomainError, repositories::RepositoryError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityApplicationError {
    #[error(transparent)]
    Domain(#[from] IdentityDomainError),

    #[error("Email '{0}' is already registered")]
    EmailAlreadyExists(String),

    #[error(transparent)]
    Repository(#[from] RepositoryError),
}
