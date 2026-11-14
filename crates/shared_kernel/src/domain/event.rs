use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Marker trait for Domain Events emitted by Aggregates.
pub trait DomainEvent: Send + Sync {
    /// Canonical event name (e.g. `"identity.user_registered"`).
    fn event_type(&self) -> &'static str;

    /// Identifier of the Aggregate Root that produced this event.
    fn aggregate_id(&self) -> String;

    /// Serialized JSON representation of the event payload.
    fn payload_json(&self) -> serde_json::Value;

    /// Converts the concrete domain event into a transport-agnostic envelope.
    fn into_envelope(&self) -> DomainEventEnvelope {
        DomainEventEnvelope {
            event_id: Uuid::new_v4(),
            event_type: self.event_type().to_string(),
            aggregate_id: self.aggregate_id(),
            occurred_at: Utc::now(),
            payload: self.payload_json(),
        }
    }
}

/// Immutable envelope wrapping a Domain Event for publishing via the Event Bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEventEnvelope {
    pub event_id: Uuid,
    pub event_type: String,
    pub aggregate_id: String,
    pub occurred_at: DateTime<Utc>,
    pub payload: serde_json::Value,
}
