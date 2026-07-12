//! 레거시 프로토콜 추상화.
//!
//! 어댑터는 [`Transport`] 뒤에서 프로토콜을 모릅니다. "송장 하나를 가져온다"는
//! **의도**만 표현하고, 그것이 `GET /invoices/INV-1` 인지 `GetInvoice` SOAP
//! action 인지는 전송이 결정합니다.
//!
//! 새 프로토콜(메인프레임·배치·RPA)을 붙이려면 `Transport` 하나를 더 구현하면
//! 됩니다 — **어댑터는 건드리지 않습니다.**
//!
//! 프로토콜이 달라도 도구 이름·입출력 스키마·권한·위험 등급은 동일합니다.
//! **LLM은 레거시가 REST인지 SOAP인지 알지 못합니다.** 오류 번역만 전송마다
//! 다릅니다.

mod db;
mod memory;
mod rest;
mod soap;

use anyhow::Result;
pub use db::DbTransport;
pub use memory::MemoryTransport;
pub use rest::RestTransport;
use serde_json::{Map,
                 Value};
pub use soap::SoapTransport;

/// 레거시에 대한 의도 하나.
#[derive(Debug, Clone, Default)]
pub struct Operation {
    /// 업무 의도의 이름. 예: `get_invoice`, `create_ticket`.
    pub name: String,
    /// 자원 경로 조각. 예: `["invoices", "INV-1"]`.
    pub path: Vec<String>,
    /// 조회 조건 또는 본문.
    pub params: Map<String, Value>,
    /// 상태를 바꾸는 작업인지.
    pub write: bool,
}

impl Operation {
    pub fn read(name: &str) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn write(name: &str) -> Self {
        Self {
            name: name.into(),
            write: true,
            ..Default::default()
        }
    }

    pub fn path(mut self, segs: &[&str]) -> Self {
        self.path = segs.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn param(mut self, k: &str, v: Value) -> Self {
        self.params.insert(k.to_string(), v);
        self
    }
}

/// 레거시 시스템으로 나가는 통로.
///
/// 구현체는 **일시적 장애와 업무 오류를 구분해서** 올려야 합니다
/// ([`ai_bridge::transient`]). 이 구분이 게이트웨이의 재시도와 서킷 브레이커를
/// 좌우합니다.
#[async_trait::async_trait]
pub trait Transport: Send + Sync + std::fmt::Debug {
    /// 의도를 실행하고 결과를 돌려줍니다.
    async fn call(&self, op: &Operation) -> Result<Value>;
    /// 도달 가능한지 확인합니다.
    async fn health(&self) -> Result<()>;
    /// 운영 콘솔에 보여줄 설명. 예: `rest https://erp.example`.
    fn describe(&self) -> String;
}

/// 자원이 없음 — **업무 오류**입니다. 재시도해도 결과가 같습니다.
#[derive(Debug, Clone)]
pub struct NotFound {
    pub what: String,
}

impl std::fmt::Display for NotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{} 을(를) 찾을 수 없습니다", self.what) }
}

impl std::error::Error for NotFound {}

pub fn not_found(what: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(NotFound {
        what: what.into(),
    })
}
