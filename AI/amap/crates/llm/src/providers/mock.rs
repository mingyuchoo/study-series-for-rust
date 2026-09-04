//! Deterministic mock provider: fixture-driven answers keyed by task, for tests and offline demos.
use crate::{LlmClient, LlmError, LlmRequest, LlmResponse, TaskKind};
use amap_domain::ModelProvider;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type Handler = Arc<dyn Fn(&LlmRequest) -> Value + Send + Sync>;

#[derive(Default)]
pub struct MockProvider {
    handlers: Mutex<HashMap<TaskKind, Handler>>,
    calls: Mutex<Vec<LlmRequest>>,
    label: String,
}

impl MockProvider {
    pub fn new() -> Self {
        Self {
            label: "mock".into(),
            ..Default::default()
        }
    }
    pub fn labelled(label: &str) -> Self {
        Self {
            label: label.into(),
            ..Default::default()
        }
    }
    pub fn on(
        self,
        task: TaskKind,
        f: impl Fn(&LlmRequest) -> Value + Send + Sync + 'static,
    ) -> Self {
        self.handlers.lock().unwrap().insert(task, Arc::new(f));
        self
    }
    pub fn on_fixture(self, task: TaskKind, fixture: Value) -> Self {
        self.on(task, move |_| fixture.clone())
    }
    pub fn calls(&self) -> Vec<LlmRequest> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl LlmClient for MockProvider {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        self.calls.lock().unwrap().push(req.clone());
        let handler = self.handlers.lock().unwrap().get(&req.task).cloned();
        let json = match handler {
            Some(h) => h(&req),
            None => json!({ "mock": true, "task": format!("{:?}", req.task) }),
        };
        Ok(LlmResponse {
            text: json.to_string(),
            json: Some(json),
            provider: ModelProvider::Mock,
            model: self.label.clone(),
            input_tokens: (req.prompt.len() / 4) as u64,
            output_tokens: 64,
            stop_reason: "end_turn".into(),
            cached: false,
        })
    }
}
