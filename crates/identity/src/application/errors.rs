use crate::domain::{errors::IdentityDomainError, repositories::RepositoryError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityApplicationError {
    #[error(transparent)]
    Domain(#[from] IdentityDomainError),

    #[error("Email '{0}' is already registered")]
    EmailAlreadyExists(String),

    #[error("Password hashing error")]
    PasswordHashing,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Invalid or expired refresh token")]
    InvalidRefreshToken,

    #[error("Invalid token")]
    InvalidToken,

    #[error("Token generation error")]
    TokenGeneration,

    #[error(transparent)]
    Repository(#[from] RepositoryError),
}
