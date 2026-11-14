use crate::domain::value_objects::{Email, FullName, UserId};
use chrono::{DateTime, Utc};

/// `User` Aggregate Root in the Identity Bounded Context.
#[derive(Debug, Clone)]
pub struct User {
    id: UserId,
    email: Email,
    full_name: FullName,
    password_hash: String,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl User {
    /// Factory method to register a new active `User` aggregate.
    pub fn register(email: Email, full_name: FullName, password_hash: impl Into<String>) -> Self {
        let id = UserId::new();
        let now = Utc::now();

        Self {
            id,
            email,
            full_name,
            password_hash: password_hash.into(),
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// Reconstitutes a `User` aggregate from persistence.
    pub fn reconstitute(
        id: UserId,
        email: Email,
        full_name: FullName,
        password_hash: impl Into<String>,
        is_active: bool,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            email,
            full_name,
            password_hash: password_hash.into(),
            is_active,
            created_at,
            updated_at,
        }
    }

    pub fn id(&self) -> UserId {
        self.id
    }

    pub fn email(&self) -> &Email {
        &self.email
    }

    pub fn full_name(&self) -> &FullName {
        &self.full_name
    }

    pub fn password_hash(&self) -> &str {
        &self.password_hash
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_registration_sets_active_state() {
        let email = Email::parse("siti@example.com").unwrap();
        let name = FullName::parse("Siti Aminah").unwrap();
        let user = User::register(email, name, "hashed_pw");

        assert!(user.is_active());
    }
}
