use crate::domain::{
    events::UserRegisteredEvent,
    value_objects::{Email, FullName, UserId},
};
use chrono::{DateTime, Utc};
use shared_kernel::domain::{DomainEvent, DomainEventEnvelope};

/// `User` Aggregate Root in the Identity Bounded Context.
#[derive(Debug, Clone)]
pub struct User {
    id: UserId,
    email: Email,
    full_name: FullName,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    domain_events: Vec<DomainEventEnvelope>,
}

impl User {
    /// Factory method to register a new active `User` aggregate and emit `UserRegisteredEvent`.
    pub fn register(email: Email, full_name: FullName) -> Self {
        let id = UserId::new();
        let now = Utc::now();

        let event = UserRegisteredEvent {
            user_id: id.as_uuid(),
            email: email.as_str().to_string(),
            full_name: full_name.as_str().to_string(),
        };

        Self {
            id,
            email,
            full_name,
            is_active: true,
            created_at: now,
            updated_at: now,
            domain_events: vec![event.into_envelope()],
        }
    }

    /// Reconstitutes a `User` aggregate from persistence without emitting new creation events.
    pub fn reconstitute(
        id: UserId,
        email: Email,
        full_name: FullName,
        is_active: bool,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            email,
            full_name,
            is_active,
            created_at,
            updated_at,
            domain_events: Vec::new(),
        }
    }

    /// Drains uncommitted domain events accumulated by this aggregate instance.
    pub fn pull_domain_events(&mut self) -> Vec<DomainEventEnvelope> {
        std::mem::take(&mut self.domain_events)
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
    fn user_registration_emits_domain_event() {
        let email = Email::parse("siti@example.com").unwrap();
        let name = FullName::parse("Siti Aminah").unwrap();
        let mut user = User::register(email, name);

        assert!(user.is_active());
        let events = user.pull_domain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "identity.user_registered");
    }
}
