use crate::domain::value_objects::UserId;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RefreshToken {
    id: Uuid,
    user_id: UserId,
    token_hash: String,
    expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl RefreshToken {
    pub fn new(user_id: UserId, token_hash: impl Into<String>, expires_at: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            token_hash: token_hash.into(),
            expires_at,
            revoked_at: None,
            created_at: Utc::now(),
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn user_id(&self) -> UserId {
        self.user_id
    }

    pub fn token_hash(&self) -> &str {
        &self.token_hash
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    pub fn revoked_at(&self) -> Option<DateTime<Utc>> {
        self.revoked_at
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn refresh_token_new_starts_unrevoked() {
        let user_id = UserId::new();
        let expires_at = Utc::now() + Duration::days(7);
        let token = RefreshToken::new(user_id, "abc123hash", expires_at);

        assert_eq!(token.user_id(), user_id);
        assert_eq!(token.token_hash(), "abc123hash");
        assert_eq!(token.expires_at(), expires_at);
        assert!(token.revoked_at().is_none());
    }
}
