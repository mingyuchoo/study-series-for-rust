//! # legacies
//!
//! 레거시 시스템 어댑터 **구현**과 Transport.
//!
//! 게이트웨이 코어(`ai-bridge`)는 이 크레이트를 **알지 못합니다.** 코어는
//! [`ai_bridge::adapter::Adapter`] 계약과 [`ai_bridge::AdapterFactory`] 만
//! 알고, 바이너리가 여기 있는 구현을 주입합니다 — 그래야 "게이트웨이는 레거시가
//! REST인지 SOAP인지 모른다"가 컴파일러가 강제하는 사실이 됩니다.
//!
//! ```text
//! Adapter (계약, ai-bridge)                        ← 게이트웨이가 아는 것
//!    ↑ 구현
//! erp · crm · ticket · docs · purchase · refund    (여기)
//!    ↓ 사용
//! Transport (REST · SOAP · DB · memory)            (여기)
//! ```

pub mod adapter;
pub mod legacy;
pub mod retriever;

use adapter::{CrmAdapter,
              DocsAdapter,
              ErpAdapter,
              PurchaseAdapter,
              RefundAdapter,
              TicketAdapter};
use ai_bridge::{AdapterFactory,
                SystemsOptions,
                adapter::Adapter,
                inventory::{Interface,
                            System},
                registry::Registry};
use anyhow::{Result,
             anyhow};
use legacy::{DbTransport,
             RestTransport,
             SoapTransport,
             Transport};
use std::sync::Arc;

/// 인벤토리와 플래그를 보고 어댑터를 조립합니다.
///
/// **`systems.yaml` 의 `interface` 와 `base_url` 이 어느 전송을 쓸지
/// 결정합니다.** `interface: rest` 를 `soap` 으로 바꾸면 전송만 바뀌고 도구
/// 이름·스키마·권한·위험 등급은 그대로입니다.
#[derive(Debug, Default)]
pub struct Systems;

impl Systems {
    pub fn new() -> Self { Self }
}

/// ERP 전송을 고릅니다. 우선순위: `-erp-db` > `-erp-url` > 인벤토리 > 인메모리.
async fn erp_transport(sys: Option<&System>, opts: &SystemsOptions) -> Result<Option<Arc<dyn Transport>>> {
    if !opts.erp_db_dsn.is_empty() {
        let t = DbTransport::open(&opts.erp_db_dsn)
            .await
            .map_err(|e| anyhow!("erp: open db {}: {e}", opts.erp_db_dsn))?;
        return Ok(Some(Arc::new(t)));
    }
    if !opts.erp_base_url.is_empty() {
        return Ok(Some(Arc::new(RestTransport::new(&opts.erp_base_url)?)));
    }
    if let Some(s) = sys
        && !s.base_url.is_empty()
    {
        return Ok(Some(match s.interface {
            | Interface::Rest => Arc::new(RestTransport::new(&s.base_url)?) as Arc<dyn Transport>,
            | Interface::Soap => Arc::new(SoapTransport::new(&s.base_url)?) as Arc<dyn Transport>,
            | other => {
                return Err(anyhow!("erp: interface {other} 에는 base_url 을 쓸 수 없습니다"));
            },
        }));
    }
    // 인메모리 — `-allow-mock-backends` 없이는 bootstrap 이 기동을 거부합니다.
    Ok(None)
}

/// 인벤토리가 REST/SOAP 을 지정했으면 그 전송을, 아니면 인메모리를 씁니다.
fn generic_transport(sys: Option<&System>) -> Result<Option<Arc<dyn Transport>>> {
    let Some(s) = sys else {
        return Ok(None);
    };
    if s.base_url.is_empty() {
        return Ok(None);
    }
    Ok(Some(match s.interface {
        | Interface::Rest => Arc::new(RestTransport::new(&s.base_url)?) as Arc<dyn Transport>,
        | Interface::Soap => Arc::new(SoapTransport::new(&s.base_url)?) as Arc<dyn Transport>,
        | _ => return Ok(None),
    }))
}

/// 어댑터 여섯을 조립합니다. 워크플로 저장소를 주면 환불 어댑터가 그것을
/// 씁니다.
///
/// `bootstrap` 은 감사·승인과 **같은 백엔드**의 워크플로 저장소를 넘겨야 합니다
/// — 승인은 PostgreSQL 에, 그 승인으로 실행되는 업무 흐름은 SQLite 에 두는
/// 식으로 갈라지면 분산 배포에서 한쪽만 공유됩니다.
pub async fn build_adapters(opts: &SystemsOptions, workflow_store: Option<Arc<dyn ai_bridge::workflow::Store>>) -> Result<Vec<Arc<dyn Adapter>>> {
    let inv = opts.inventory.clone();
    let sys = |name: &str| inv.as_ref().and_then(|i| i.lookup(name));

    let mut out: Vec<Arc<dyn Adapter>> = Vec::new();

    // --- ERP: 인메모리 · REST · SOAP · DB 를 **같은 어댑터 코드**로 씁니다 ---
    out.push(Arc::new(match erp_transport(sys("erp").as_ref(), opts).await? {
        | Some(t) => ErpAdapter::new(t),
        | None => ErpAdapter::in_memory(),
    }));

    out.push(Arc::new(match generic_transport(sys("crm").as_ref())? {
        | Some(t) => CrmAdapter::new(t),
        | None => CrmAdapter::in_memory(),
    }));

    out.push(Arc::new(match generic_transport(sys("ticket").as_ref())? {
        | Some(t) => TicketAdapter::new(t),
        | None => TicketAdapter::in_memory(),
    }));

    // --- Docs (RAG) — 검색기는 교체 가능합니다 ---
    out.push(Arc::new(match &opts.docs_retriever {
        | Some(r) => DocsAdapter::new(r.clone()).await?,
        | None => DocsAdapter::in_memory().await?,
    }));

    out.push(Arc::new(match generic_transport(sys("purchase").as_ref())? {
        | Some(t) => PurchaseAdapter::new(t),
        | None => PurchaseAdapter::in_memory(),
    }));

    // --- Refund (워크플로) ---
    let store = workflow_store.unwrap_or_else(|| Arc::new(ai_bridge::workflow::MemoryStore::new()));
    out.push(Arc::new(RefundAdapter::new(store)));

    Ok(out)
}

#[async_trait::async_trait]
impl AdapterFactory for Systems {
    async fn adapters(&self, opts: &SystemsOptions) -> Result<Vec<Arc<dyn Adapter>>> { build_adapters(opts, None).await }

    async fn rebind(&self, reg: &Registry, opts: &SystemsOptions) -> Result<()> {
        // 인벤토리가 바뀌면 어댑터를 다시 만들고 **도구 핸들러만 갈아끼웁니다.**
        // 도구 이름·스키마·권한·위험 등급은 그대로이므로 MCP 클라이언트가 보는 계약은
        // 변하지 않습니다 — 뒤에 붙은 전송만 바뀝니다.
        for a in self.adapters(opts).await? {
            for tool in a.tools() {
                reg.replace(tool)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_bridge::inventory::Inventory;
    use std::path::Path;

    #[tokio::test]
    async fn assembles_all_six_systems() {
        let adapters = build_adapters(&SystemsOptions::default(), None).await.unwrap();
        let mut names: Vec<String> = adapters.iter().map(|a| a.name()).collect();
        names.sort();
        assert_eq!(names, vec!["crm", "docs", "erp", "purchase", "refund", "ticket"]);
    }

    /// README 의 도구 표와 정확히 일치해야 합니다.
    #[tokio::test]
    async fn exposes_exactly_the_eleven_documented_tools() {
        let adapters = build_adapters(&SystemsOptions::default(), None).await.unwrap();
        let reg = Registry::new();
        for a in &adapters {
            for t in a.tools() {
                reg.register(t).unwrap();
            }
        }
        let mut names = reg.names();
        names.sort();
        assert_eq!(
            names,
            vec![
                "create_support_ticket",
                "draft_purchase_request",
                "get_customer_invoices",
                "get_customer_profile",
                "get_invoice_status",
                "get_ticket_status",
                "get_workflow_status",
                "process_refund",
                "search_contracts",
                "search_documents",
                "submit_purchase_request",
            ]
        );
    }

    /// 인벤토리가 `base_url` 을 주면 전송이 바뀝니다 — **도구 계약은
    /// 그대로입니다.**
    #[tokio::test]
    async fn switching_the_transport_does_not_change_the_tool_contract() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/systems.yaml");
        let inv = Arc::new(Inventory::load(&path).unwrap());

        let contract_of = |adapters: &[Arc<dyn Adapter>]| {
            adapters
                .iter()
                .find(|a| a.name() == "erp")
                .unwrap()
                .tools()
                .iter()
                .map(|t| (t.spec.name.clone(), t.spec.risk_level, t.spec.access))
                .collect::<Vec<_>>()
        };

        let memory = build_adapters(&SystemsOptions::default(), None).await.unwrap();

        let rest_opts = SystemsOptions {
            inventory: Some(inv),
            erp_base_url: "https://erp.example.com".into(),
            ..Default::default()
        };
        let rest = build_adapters(&rest_opts, None).await.unwrap();

        // **LLM 은 레거시가 메모리인지 REST인지 알지 못합니다.**
        assert_eq!(contract_of(&memory), contract_of(&rest));
    }
}
