use crate::domain::errors::IdentityDomainError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Value Object representing a validated full name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullName(String);

impl FullName {
    pub fn parse(raw: impl Into<String>) -> Result<Self, IdentityDomainError> {
        let trimmed = raw.into().trim().to_string();
        let char_count = trimmed.chars().count();
        if !(2..=150).contains(&char_count) {
            return Err(IdentityDomainError::InvalidFullName);
        }
        Ok(Self(trimmed))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FullName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
