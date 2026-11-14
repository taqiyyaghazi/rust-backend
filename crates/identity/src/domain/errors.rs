use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IdentityDomainError {
    #[error("Invalid email format: '{0}'")]
    InvalidEmail(String),

    #[error("Full name must be between 2 and 150 characters")]
    InvalidFullName,

    #[error("Password must be between 8 and 128 characters")]
    InvalidPassword,
}
