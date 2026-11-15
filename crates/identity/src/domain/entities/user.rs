use crate::domain::value_objects::{Email, FullName, UserId};
use chrono::{DateTime, Utc};

/// `User` Aggregate Root in the Identity Bounded Context.
#[derive(Clone)]
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

    pub fn reconstitute(
        id: UserId,
        email: Email,
        full_name: FullName,
        password_hash: String,
        is_active: bool,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            email,
            full_name,
            password_hash,
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

    #[test]
    fn user_reconstitute_preserves_fields() {
        let id = UserId::new();
        let email = Email::parse("siti@example.com").unwrap();
        let name = FullName::parse("Siti Aminah").unwrap();
        let now = Utc::now();
        let user = User::reconstitute(
            id,
            email.clone(),
            name.clone(),
            "hash".into(),
            false,
            now,
            now,
        );

        assert_eq!(user.id(), id);
        assert_eq!(user.email().as_str(), email.as_str());
        assert_eq!(user.full_name().as_str(), name.as_str());
        assert_eq!(user.password_hash(), "hash");
        assert!(!user.is_active());
        assert_eq!(user.created_at(), now);
        assert_eq!(user.updated_at(), now);
    }
}
