//! Control-plane event bus: `build.request`, `verification.failed`, `human.review.required`, …
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Mutex;
use tokio::sync::broadcast;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub subject: String,
    pub run_id: String,
    pub function_id: String,
    pub payload: Value,
    pub at: DateTime<Utc>,
}

#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: Event);
    fn subscribe(&self) -> broadcast::Receiver<Event>;
    /// Events retained in memory (for the control-plane API / tests).
    fn history(&self) -> Vec<Event>;
}

pub struct InMemoryBus {
    tx: broadcast::Sender<Event>,
    history: Mutex<Vec<Event>>,
}

impl Default for InMemoryBus {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(4096);
        Self {
            tx,
            history: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl EventBus for InMemoryBus {
    async fn publish(&self, event: Event) {
        tracing::debug!(subject = %event.subject, run = %event.run_id, "event");
        self.history.lock().unwrap().push(event.clone());
        let _ = self.tx.send(event);
    }
    fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
    fn history(&self) -> Vec<Event> {
        self.history.lock().unwrap().clone()
    }
}

/// NATS JetStream-backed bus. Subjects are prefixed with `amap.` and mirrored locally.
#[cfg(feature = "nats")]
pub struct NatsBus {
    client: async_nats::Client,
    local: InMemoryBus,
    prefix: String,
}

#[cfg(feature = "nats")]
impl NatsBus {
    pub async fn connect(url: &str, prefix: &str) -> Result<Self, async_nats::Error> {
        let client = async_nats::connect(url).await?;
        // Ensure a JetStream stream exists for durable control events (best effort).
        let js = async_nats::jetstream::new(client.clone());
        let _ = js
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name: "AMAP_CONTROL".into(),
                subjects: vec![format!("{prefix}.>")],
                ..Default::default()
            })
            .await;
        Ok(Self {
            client,
            local: InMemoryBus::new(),
            prefix: prefix.to_string(),
        })
    }
}

#[cfg(feature = "nats")]
#[async_trait]
impl EventBus for NatsBus {
    async fn publish(&self, event: Event) {
        let subject = format!("{}.{}", self.prefix, event.subject);
        if let Ok(bytes) = serde_json::to_vec(&event) {
            if let Err(e) = self.client.publish(subject, bytes.into()).await {
                tracing::warn!(error = %e, "nats publish failed");
            }
        }
        self.local.publish(event).await;
    }
    fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.local.subscribe()
    }
    fn history(&self) -> Vec<Event> {
        self.local.history()
    }
}
