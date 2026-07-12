//! 인메모리 전송 (개발·데모용).
//!
//! 어댑터가 자기 목 데이터를 들고 이 전송을 만듭니다. 도구
//! 이름·스키마·권한·위험 등급은 실제 백엔드와 **동일**하므로, `-erp-url` 하나만
//! 바꾸면 같은 도구가 진짜 ERP를 부릅니다.
//!
//! **운영에서는 `-allow-mock-backends` 없이 기동이 거부됩니다.**

use super::{Operation,
            Transport};
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

/// 의도를 처리하는 목 백엔드.
#[async_trait::async_trait]
pub trait MemoryBackend: Send + Sync {
    async fn call(&self, op: &Operation) -> Result<Value>;
}

#[async_trait::async_trait]
impl<F, Fut> MemoryBackend for F
where
    F: Fn(Operation) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<Value>> + Send,
{
    async fn call(&self, op: &Operation) -> Result<Value> { (self)(op.clone()).await }
}

/// 인메모리 전송.
pub struct MemoryTransport {
    system: String,
    backend: Arc<dyn MemoryBackend>,
}

impl std::fmt::Debug for MemoryTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("MemoryTransport").field("system", &self.system).finish() }
}

impl MemoryTransport {
    pub fn new(system: &str, backend: Arc<dyn MemoryBackend>) -> Self {
        Self {
            system: system.to_string(),
            backend,
        }
    }
}

#[async_trait::async_trait]
impl Transport for MemoryTransport {
    async fn call(&self, op: &Operation) -> Result<Value> { self.backend.call(op).await }

    async fn health(&self) -> Result<()> {
        Ok(()) // 프로세스 메모리는 언제나 살아 있습니다.
    }

    fn describe(&self) -> String { format!("memory ({})", self.system) }
}
