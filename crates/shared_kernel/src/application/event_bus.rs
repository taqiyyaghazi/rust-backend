use crate::domain::DomainEventEnvelope;
use async_trait::async_trait;
use std::sync::Arc;

/// Subscriber contract for handling Domain Events across Bounded Contexts.
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Name of the subscriber for observability/tracing.
    fn subscriber_name(&self) -> &'static str;

    /// Handles an incoming domain event envelope.
    async fn on_event(&self, event: &DomainEventEnvelope);
}

/// Application port for publishing Domain Events emitted by Aggregates.
#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: DomainEventEnvelope);

    async fn publish_all(&self, events: Vec<DomainEventEnvelope>) {
        for event in events {
            self.publish(event).await;
        }
    }

    async fn register_subscriber(&self, subscriber: Arc<dyn EventSubscriber>);
}
