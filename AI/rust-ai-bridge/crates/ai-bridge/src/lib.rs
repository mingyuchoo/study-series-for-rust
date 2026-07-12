//! # ai-bridge
//!
//! 레거시 IT 시스템과 LLM AI 에이전트를 안전하게 연결하는 **AI Integration
//! Gateway**.
//!
//! 핵심 원칙: **LLM은 판단하고, 게이트웨이는 집행한다.** LLM은 "무엇을 하고
//! 싶은지"를 도구 호출로 제안할 뿐이고, 실제 실행은 게이트웨이가 통제합니다.
//!
//! 모든 도구 호출은 `gateway` 의 파이프라인을 통과합니다 — 레이트 리밋 · 비용
//! 상한 · allowlist · 입력 검증 · 정책(RBAC+ABAC) · 고위험 차단 · 승인 관문 ·
//! 어댑터 실행 · 출력 검증 · 의무 집행 · PII 마스킹 · 인젝션 표시 · 감사 로그.
//!
//! 어댑터 구현(ERP·CRM 등)은 이 크레이트에 없습니다. [`adapter::Adapter`]
//! 계약과 [`AdapterFactory`] 만 여기에 두고, 구현은 `legacies` 크레이트가
//! 제공합니다 — 게이트웨이 코어가 레거시 구현을 모르게 하기 위함입니다.

pub mod adapter;
pub mod app;
pub mod approval;
pub mod audit;
pub mod auth;
pub mod breaker;
pub mod budget;
pub mod clock;
pub mod console;
pub mod eval;
pub mod gateway;
pub mod injection;
pub mod inventory;
pub mod llm;
pub mod mcpserver;
pub mod ops;
pub mod pii;
pub mod policy;
pub mod ratelimit;
pub mod registry;
pub mod schema;
pub mod storetest;
pub mod telemetry;
pub mod toolcatalog;
pub mod transient;
pub mod workflow;

use std::sync::Arc;

/// 어댑터 조립 옵션. Go 의 `systems.Options` 대응.
#[derive(Clone, Default)]
pub struct SystemsOptions {
    /// 인벤토리 — `interface` 와 `base_url` 이 어느 백엔드를 쓸지 결정합니다.
    pub inventory: Option<Arc<inventory::Inventory>>,
    /// ERP REST base URL 강제 지정 (`-erp-url`). 인벤토리보다 우선합니다.
    pub erp_base_url: String,
    /// ERP 관계형 DB DSN (`-erp-db`). `erp_base_url` 보다 우선합니다.
    pub erp_db_dsn: String,
    /// RAG 검색기 (`-docs-retriever`).
    pub docs_retriever: Option<Arc<dyn adapter::Retriever>>,
}

impl std::fmt::Debug for SystemsOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemsOptions")
            .field("erp_base_url", &self.erp_base_url)
            .field("erp_db_dsn", &self.erp_db_dsn)
            .finish()
    }
}

/// 레거시 어댑터 집합을 만드는 공장.
///
/// 게이트웨이 코어(`bootstrap`·`auditcli`·`eval::golden`)는 어댑터 **구현**을
/// 알지 못한 채 조립만 합니다. Go 에서는 형제 모듈 `go-legacies` 를 import 해
/// 이 역할을 했지만, Rust 에서 그렇게 하면 `legacies → ai-bridge → legacies`
/// 순환이 됩니다. 그래서 의존을 뒤집어, 구현이 이 트레이트를 만족시키고
/// 바이너리가 주입합니다.
#[async_trait::async_trait]
pub trait AdapterFactory: Send + Sync {
    /// 인벤토리·플래그에 맞춰 어댑터를 조립합니다.
    async fn adapters(&self, opts: &SystemsOptions) -> anyhow::Result<Vec<Arc<dyn adapter::Adapter>>>;

    /// 인벤토리가 바뀐 뒤 이미 등록된 도구의 핸들러를 재배선합니다.
    ///
    /// 인벤토리에서 `base_url` 이나 `interface` 가 바뀌면 도구 이름·스키마는
    /// 그대로 두고 뒤에 붙은 전송만 갈아끼웁니다. 콘솔의 인벤토리 Apply 와
    /// SIGHUP 이 씁니다.
    async fn rebind(&self, reg: &registry::Registry, opts: &SystemsOptions) -> anyhow::Result<()>;
}
