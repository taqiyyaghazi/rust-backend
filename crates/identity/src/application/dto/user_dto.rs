use crate::domain::entities::User;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Data Transfer Object representing a User across Application & Presentation boundaries.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserDto {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&User> for UserDto {
    fn from(user: &User) -> Self {
        Self {
            id: user.id().as_uuid(),
            email: user.email().as_str().to_string(),
            full_name: user.full_name().as_str().to_string(),
            is_active: user.is_active(),
            created_at: user.created_at(),
            updated_at: user.updated_at(),
        }
    }
}
