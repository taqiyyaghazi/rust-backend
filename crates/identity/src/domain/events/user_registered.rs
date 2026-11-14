use serde::{Deserialize, Serialize};
use shared_kernel::domain::DomainEvent;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRegisteredEvent {
    pub user_id: Uuid,
    pub email: String,
    pub full_name: String,
}

impl DomainEvent for UserRegisteredEvent {
    fn event_type(&self) -> &'static str {
        "identity.user_registered"
    }

    fn aggregate_id(&self) -> String {
        self.user_id.to_string()
    }

    fn payload_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}
