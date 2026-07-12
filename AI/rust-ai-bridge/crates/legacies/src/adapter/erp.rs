//! ERP 어댑터 — 송장·결제.
//!
//! **같은 코드로 memory · REST · SOAP · DB 를 씁니다.** 도구
//! 이름·스키마·권한·위험 등급은 백엔드와 무관하게 동일합니다 — LLM 은 레거시가
//! 무엇인지 알지 못합니다.

use crate::legacy::{MemoryTransport,
                    Operation,
                    Transport,
                    not_found};
use ai_bridge::{adapter::{Adapter,
                          array_prop,
                          int_prop,
                          object,
                          str_prop},
                auth::Identity,
                registry::{Access,
                           RiskLevel,
                           Sensitivity,
                           Spec,
                           Tool,
                           handler}};
use anyhow::Result;
use serde_json::{Map,
                 Value,
                 json};
use std::sync::Arc;

/// 송장 하나의 출력 스키마.
fn invoice_schema() -> Value {
    object(
        vec![
            ("invoice_id", str_prop("송장 번호")),
            ("customer_id", str_prop("고객 ID")),
            ("status", str_prop("결제 상태 (paid | unpaid | overdue)")),
            ("amount", int_prop("금액(원)")),
            ("issued_at", str_prop("발행일")),
            ("paid_at", str_prop("결제일")),
        ],
        &["invoice_id", "status"],
    )
}

pub struct ErpAdapter {
    transport: Arc<dyn Transport>,
}

impl ErpAdapter {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self {
            transport,
        }
    }

    /// 개발·데모용 인메모리 ERP.
    pub fn in_memory() -> Self {
        Self::new(Arc::new(MemoryTransport::new(
            "erp",
            Arc::new(|op: Operation| async move { memory_backend(&op).await }),
        )))
    }
}

async fn memory_backend(op: &Operation) -> Result<Value> {
    let invoices = mock_invoices();
    match op.name.as_str() {
        | "get_invoice" => {
            let id = op.path.last().cloned().unwrap_or_default();
            invoices
                .iter()
                .find(|i| i["invoice_id"] == json!(id))
                .cloned()
                // "없는 송장"은 업무 오류입니다 — 재시도해도 결과가 같습니다.
                .ok_or_else(|| not_found(format!("송장 {id}")))
        },
        | "list_customer_invoices" => {
            let cid = op.params.get("customer_id").and_then(|v| v.as_str()).unwrap_or_default();
            let mine: Vec<Value> = invoices.into_iter().filter(|i| i["customer_id"] == json!(cid)).collect();
            Ok(json!({ "invoices": mine }))
        },
        | other => Err(anyhow::anyhow!("erp: unsupported operation {other:?}")),
    }
}

fn mock_invoices() -> Vec<Value> {
    vec![
        json!({"invoice_id":"INV-2026-0001","customer_id":"CUST-1001","status":"paid",
               "amount":1_200_000,"issued_at":"2026-06-01","paid_at":"2026-06-15"}),
        json!({"invoice_id":"INV-2026-0002","customer_id":"CUST-1001","status":"unpaid",
               "amount":450_000,"issued_at":"2026-06-20","paid_at":""}),
        json!({"invoice_id":"INV-2026-0003","customer_id":"CUST-1002","status":"overdue",
               "amount":2_300_000,"issued_at":"2026-05-02","paid_at":""}),
        json!({"invoice_id":"INV-2026-0004","customer_id":"CUST-2001","status":"paid",
               "amount":780_000,"issued_at":"2026-06-11","paid_at":"2026-06-12"}),
    ]
}

#[async_trait::async_trait]
impl Adapter for ErpAdapter {
    fn name(&self) -> String { "erp".into() }

    async fn health_check(&self) -> Result<()> { self.transport.health().await }

    fn tools(&self) -> Vec<Tool> {
        let t1 = self.transport.clone();
        let t2 = self.transport.clone();

        vec![
            Tool {
                spec: Spec {
                    name: "get_invoice_status".into(),
                    description: "송장 번호로 결제 상태를 조회합니다.".into(),
                    system: "erp".into(),
                    access: Access::Read,
                    risk_level: RiskLevel::L1,
                    sensitivity: Sensitivity::Confidential,
                    required_permissions: vec!["erp.invoice.read".into()],
                    rate_limit_per_min: 60,
                    timeout_ms: 5_000,
                    // 읽기이므로 재시도할 수 있습니다.
                    max_retries: 2,
                    log_retention_days: 365,
                    fallback: "재무팀(finance-help@example.com)에 문의하세요.".into(),
                    input_schema: object(vec![("invoice_id", str_prop("송장 번호. 예: INV-2026-0001"))], &["invoice_id"]),
                    output_schema: invoice_schema(),
                    ..Default::default()
                },
                handler: handler(move |_id: Identity, args: Map<String, Value>| {
                    let t = t1.clone();
                    async move {
                        let invoice_id = args.get("invoice_id").and_then(|v| v.as_str()).unwrap_or_default();
                        let op = Operation::read("get_invoice").path(&["invoices", invoice_id]);
                        t.call(&op).await
                    }
                }),
            },
            Tool {
                spec: Spec {
                    name: "get_customer_invoices".into(),
                    description: "고객의 송장 목록을 조회합니다.".into(),
                    system: "erp".into(),
                    access: Access::Read,
                    risk_level: RiskLevel::L1,
                    sensitivity: Sensitivity::Confidential,
                    required_permissions: vec!["erp.invoice.read".into()],
                    rate_limit_per_min: 30,
                    timeout_ms: 5_000,
                    max_retries: 2,
                    log_retention_days: 365,
                    fallback: "재무팀(finance-help@example.com)에 문의하세요.".into(),
                    input_schema: object(vec![("customer_id", str_prop("고객 ID. 예: CUST-1001"))], &["customer_id"]),
                    output_schema: object(vec![("invoices", array_prop("송장 목록", invoice_schema()))], &["invoices"]),
                    ..Default::default()
                },
                handler: handler(move |_id: Identity, args: Map<String, Value>| {
                    let t = t2.clone();
                    async move {
                        let cid = args.get("customer_id").and_then(|v| v.as_str()).unwrap_or_default();
                        let op = Operation::read("list_customer_invoices").path(&["invoices"]).param("customer_id", json!(cid));
                        t.call(&op).await
                    }
                }),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_bridge::transient;

    fn args(v: Value) -> Map<String, Value> { v.as_object().unwrap().clone() }

    #[tokio::test]
    async fn looks_up_an_invoice() {
        let a = ErpAdapter::in_memory();
        let tools = a.tools();
        let t = tools.iter().find(|t| t.spec.name == "get_invoice_status").unwrap();

        let out = t
            .handler
            .call(&Identity::default(), &args(json!({"invoice_id":"INV-2026-0001"})))
            .await
            .unwrap();
        assert_eq!(out["status"], json!("paid"));
        assert_eq!(out["amount"], json!(1_200_000));
    }

    #[tokio::test]
    async fn a_missing_invoice_is_a_business_error_not_a_transient_one() {
        // 없는 송장을 조회했다고 ERP 회로가 열리면 안 됩니다.
        let a = ErpAdapter::in_memory();
        let tools = a.tools();
        let t = tools.iter().find(|t| t.spec.name == "get_invoice_status").unwrap();

        let err = t.handler.call(&Identity::default(), &args(json!({"invoice_id":"INV-9999"}))).await.unwrap_err();
        assert!(!transient::is_temporary(&err));
    }

    #[tokio::test]
    async fn lists_invoices_for_a_customer() {
        let a = ErpAdapter::in_memory();
        let tools = a.tools();
        let t = tools.iter().find(|t| t.spec.name == "get_customer_invoices").unwrap();

        let out = t.handler.call(&Identity::default(), &args(json!({"customer_id":"CUST-1001"}))).await.unwrap();
        assert_eq!(out["invoices"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn read_tools_may_retry_but_are_l1() {
        let a = ErpAdapter::in_memory();
        for t in a.tools() {
            assert_eq!(t.spec.access, Access::Read);
            assert_eq!(t.spec.risk_level, RiskLevel::L1);
            assert!(t.spec.max_retries > 0);
        }
    }
}
