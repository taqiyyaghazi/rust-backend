use crate::application::{EventBus, EventSubscriber};
use crate::domain::DomainEventEnvelope;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Thread-safe in-process implementation of `EventBus`.
#[derive(Default, Clone)]
pub struct InMemoryEventBus {
    subscribers: Arc<RwLock<Vec<Arc<dyn EventSubscriber>>>>,
    published_history: Arc<RwLock<Vec<DomainEventEnvelope>>>,
}

impl InMemoryEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a snapshot of published domain events (useful for testing and diagnostics).
    pub async fn published_events(&self) -> Vec<DomainEventEnvelope> {
        self.published_history.read().await.clone()
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish(&self, event: DomainEventEnvelope) {
        info!(
            event_id = %event.event_id,
            event_type = %event.event_type,
            aggregate_id = %event.aggregate_id,
            "Domain event published"
        );

        self.published_history.write().await.push(event.clone());

        let subscribers = self.subscribers.read().await.clone();
        for subscriber in subscribers {
            subscriber.on_event(&event).await;
        }
    }

    async fn register_subscriber(&self, subscriber: Arc<dyn EventSubscriber>) {
        info!(
            subscriber = subscriber.subscriber_name(),
            "Registered domain event subscriber"
        );
        self.subscribers.write().await.push(subscriber);
    }
}

/// Built-in subscriber that records domain events to structured logs.
pub struct AuditLogEventSubscriber;

#[async_trait]
impl EventSubscriber for AuditLogEventSubscriber {
    fn subscriber_name(&self) -> &'static str {
        "AuditLogEventSubscriber"
    }

    async fn on_event(&self, event: &DomainEventEnvelope) {
        info!(
            subscriber = self.subscriber_name(),
            event_type = %event.event_type,
            aggregate_id = %event.aggregate_id,
            occurred_at = %event.occurred_at,
            payload = %event.payload,
            "Audit trail captured domain event"
        );
    }
}
