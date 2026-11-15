use crate::domain::errors::IdentityDomainError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Value Object encapsulating validated, normalized email addresses.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Email(String);

impl Email {
    pub fn parse(raw: impl Into<String>) -> Result<Self, IdentityDomainError> {
        let normalized = raw.into().trim().to_lowercase();
        if normalized.chars().count() > 255 {
            return Err(IdentityDomainError::InvalidEmail(normalized));
        }
        let parts: Vec<&str> = normalized.split('@').collect();
        if parts.len() != 2 {
            return Err(IdentityDomainError::InvalidEmail(normalized));
        }

        let (local, domain) = (parts[0], parts[1]);
        if domain.len() < 3
            || !domain.contains('.')
            || local.split('.').any(str::is_empty)
            || domain.split('.').any(str::is_empty)
            || normalized
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(IdentityDomainError::InvalidEmail(normalized));
        }

        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Email {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_validates_and_normalizes() {
        let email = Email::parse("  Budi.Santoso@Example.com ").unwrap();
        assert_eq!(email.as_str(), "budi.santoso@example.com");

        assert!(Email::parse("invalid-email").is_err());
        assert!(Email::parse("@example.com").is_err());
        assert!(Email::parse("user@localhost").is_err());
        assert!(Email::parse("budi santoso@example.com").is_err());
        assert!(Email::parse("user@\texample.com").is_err());
        assert!(Email::parse("user@a..com").is_err());
        assert!(Email::parse(".user@example.com").is_err());
        assert!(Email::parse(format!("{}@example.com", "a".repeat(250))).is_err());
    }
}
