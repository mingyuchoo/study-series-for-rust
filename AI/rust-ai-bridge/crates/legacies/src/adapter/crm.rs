//! CRM 어댑터 — 고객·계약.
//!
//! 이 어댑터의 출력 필드가 정책 의무의 대상입니다:
//!
//! - `get_customer_profile` → `rrn`·`phone`·`email` (외부 LLM 이면 제거,
//!   고객지원팀에는 마스킹)
//! - `search_contracts` → `amount` 마스킹 · `signed_at` 제거 · 최대 3건(키워드
//!   없으면 1건)
//!
//! **행 수준 접근 제어는 어댑터가 합니다.** 정책 엔진은 "이 도구를 호출해도
//! 되는가"까지만 판단하고, 볼 수 없는 행을 걸러내는 일은 데이터를 아는 쪽만 할
//! 수 있습니다.

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

fn customer_schema() -> Value {
    object(
        vec![
            ("customer_id", str_prop("고객 ID")),
            ("name", str_prop("고객명")),
            ("rrn", str_prop("주민등록번호")),
            ("phone", str_prop("연락처")),
            ("email", str_prop("이메일")),
            ("grade", str_prop("등급")),
            ("manager", str_prop("담당자")),
        ],
        &["customer_id", "name"],
    )
}

fn contract_schema() -> Value {
    object(
        vec![
            ("contract_id", str_prop("계약 번호")),
            ("customer_id", str_prop("고객 ID")),
            ("title", str_prop("계약명")),
            ("amount", int_prop("계약 금액(원)")),
            ("signed_at", str_prop("체결일")),
        ],
        &["contract_id", "title"],
    )
}

pub struct CrmAdapter {
    transport: Arc<dyn Transport>,
}

impl CrmAdapter {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self {
            transport,
        }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(MemoryTransport::new(
            "crm",
            Arc::new(|op: Operation| async move { memory_backend(&op).await }),
        )))
    }
}

async fn memory_backend(op: &Operation) -> Result<Value> {
    match op.name.as_str() {
        | "get_customer" => {
            let id = op.path.last().cloned().unwrap_or_default();
            mock_customers()
                .into_iter()
                .find(|c| c["customer_id"] == json!(id))
                .ok_or_else(|| not_found(format!("고객 {id}")))
        },
        | "search_contracts" => {
            let keyword = op.params.get("keyword").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let hits: Vec<Value> = mock_contracts()
                .into_iter()
                .filter(|c| keyword.is_empty() || c["title"].as_str().unwrap_or("").to_lowercase().contains(&keyword))
                .collect();
            Ok(json!({ "contracts": hits }))
        },
        | other => Err(anyhow::anyhow!("crm: unsupported operation {other:?}")),
    }
}

fn mock_customers() -> Vec<Value> {
    vec![
        json!({"customer_id":"CUST-1001","name":"김철수","rrn":"900101-1234567",
               "phone":"010-1234-5678","email":"chulsoo@example.com","grade":"VIP",
               "manager":"emp-sales-01"}),
        json!({"customer_id":"CUST-1002","name":"이영희","rrn":"880202-2345678",
               "phone":"010-2345-6789","email":"younghee@example.com","grade":"일반",
               "manager":"emp-sales-01"}),
        json!({"customer_id":"CUST-2001","name":"박민수","rrn":"920303-1456789",
               "phone":"010-3456-7890","email":"minsoo@example.com","grade":"VIP",
               "manager":"emp-sales-02"}),
    ]
}

fn mock_contracts() -> Vec<Value> {
    vec![
        json!({"contract_id":"CT-2026-001","customer_id":"CUST-1001","title":"클라우드 유지보수 계약",
               "amount":48_000_000,"signed_at":"2026-01-15"}),
        json!({"contract_id":"CT-2026-002","customer_id":"CUST-1002","title":"클라우드 스토리지 증설",
               "amount":12_000_000,"signed_at":"2026-02-20"}),
        json!({"contract_id":"CT-2026-003","customer_id":"CUST-2001","title":"보안 컨설팅 계약",
               "amount":30_000_000,"signed_at":"2026-03-05"}),
        json!({"contract_id":"CT-2026-004","customer_id":"CUST-1001","title":"클라우드 백업 서비스",
               "amount":9_000_000,"signed_at":"2026-04-11"}),
        json!({"contract_id":"CT-2026-005","customer_id":"CUST-2001","title":"네트워크 재구축",
               "amount":75_000_000,"signed_at":"2026-05-30"}),
    ]
}

#[async_trait::async_trait]
impl Adapter for CrmAdapter {
    fn name(&self) -> String { "crm".into() }

    async fn health_check(&self) -> Result<()> { self.transport.health().await }

    fn tools(&self) -> Vec<Tool> {
        let t1 = self.transport.clone();
        let t2 = self.transport.clone();

        vec![
            Tool {
                spec: Spec {
                    name: "get_customer_profile".into(),
                    description: "고객 프로필을 조회합니다(민감정보는 정책에 따라 가려집니다).".into(),
                    system: "crm".into(),
                    access: Access::Read,
                    risk_level: RiskLevel::L1,
                    sensitivity: Sensitivity::Restricted,
                    required_permissions: vec!["crm.customer.read".into()],
                    rate_limit_per_min: 60,
                    timeout_ms: 5_000,
                    max_retries: 2,
                    log_retention_days: 365,
                    // 도구 명세 차원의 기본 마스킹. 정책 의무가 여기에 **더** 얹힙니다.
                    mask_fields: vec!["rrn".into()],
                    fallback: "영업지원팀(sales-support@example.com)에 접근 요청을 제출하세요.".into(),
                    input_schema: object(vec![("customer_id", str_prop("고객 ID. 예: CUST-1001"))], &["customer_id"]),
                    output_schema: customer_schema(),
                    ..Default::default()
                },
                handler: handler(move |_id: Identity, args: Map<String, Value>| {
                    let t = t1.clone();
                    async move {
                        let cid = args.get("customer_id").and_then(|v| v.as_str()).unwrap_or_default();
                        let op = Operation::read("get_customer").path(&["customers", cid]);
                        t.call(&op).await
                    }
                }),
            },
            Tool {
                spec: Spec {
                    name: "search_contracts".into(),
                    description: "계약을 키워드로 검색합니다.".into(),
                    system: "crm".into(),
                    access: Access::Read,
                    risk_level: RiskLevel::L1,
                    sensitivity: Sensitivity::Confidential,
                    required_permissions: vec!["crm.contract.read".into()],
                    rate_limit_per_min: 30,
                    timeout_ms: 5_000,
                    max_retries: 2,
                    log_retention_days: 365,
                    fallback: "영업지원팀(sales-support@example.com)에 문의하세요.".into(),
                    input_schema: object(
                        vec![("keyword", str_prop("검색어. 비우면 전체 조회"))],
                        // 필수가 아니므로 정책의 `keyword == ""` 규칙이 의미를 가집니다.
                        &[],
                    ),
                    output_schema: object(
                        vec![
                            ("contracts", array_prop("계약 목록", contract_schema())),
                            ("truncated", json!({"type":"boolean"})),
                            ("truncated_reason", str_prop("잘린 이유")),
                        ],
                        &["contracts"],
                    ),
                    ..Default::default()
                },
                handler: handler(move |id: Identity, args: Map<String, Value>| {
                    let t = t2.clone();
                    async move {
                        let keyword = args.get("keyword").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let op = Operation::read("search_contracts").path(&["contracts"]).param("keyword", json!(keyword));
                        let mut out = t.call(&op).await?;

                        // **행 수준 접근 제어.** 영업팀은 담당 고객의 계약만 봅니다.
                        // 볼 수 없는 계약은 "권한 없음"이 아니라 **아예 없는 것처럼** 처리합니다 —
                        // 건수나 제목이 새어나가는 것 자체가 정보 노출입니다.
                        if let Some(list) = out["contracts"].as_array() {
                            let visible: Vec<Value> = list.iter().filter(|c| can_see_contract(&id, c)).cloned().collect();
                            out["contracts"] = Value::Array(visible);
                        }
                        Ok(out)
                    }
                }),
            },
        ]
    }
}

/// 이 주체가 이 계약을 볼 수 있는가.
fn can_see_contract(id: &Identity, contract: &Value) -> bool {
    // 재무·관리자는 전사 조회.
    if id.has_role("finance") || id.has_role("admin") {
        return true;
    }
    // 영업팀은 담당 고객만.
    if id.has_role("sales") {
        let cid = contract["customer_id"].as_str().unwrap_or("");
        let managed = id
            .attr("managed_customers")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()).any(|m| m == cid))
            .unwrap_or(false);
        return managed;
    }
    // 그 밖의 역할은 계약을 보지 않습니다(정책이 이미 막지만, 어댑터도
    // fail-closed).
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn args(v: Value) -> Map<String, Value> { v.as_object().unwrap().clone() }

    fn sales(managed: &[&str]) -> Identity {
        Identity {
            user_id: "emp-sales-01".into(),
            roles: vec!["sales".into()],
            attributes: HashMap::from([("managed_customers".to_string(), json!(managed))]),
            ..Default::default()
        }
    }

    fn contracts_tool() -> Tool { CrmAdapter::in_memory().tools().into_iter().find(|t| t.spec.name == "search_contracts").unwrap() }

    #[tokio::test]
    async fn profile_carries_the_fields_the_policy_targets() {
        // 정책이 rrn/phone/email 을 지우거나 가리므로 어댑터가 반드시 내야 합니다.
        let a = CrmAdapter::in_memory();
        let t = a.tools().into_iter().find(|t| t.spec.name == "get_customer_profile").unwrap();
        let out = t.handler.call(&sales(&["CUST-1001"]), &args(json!({"customer_id":"CUST-1001"}))).await.unwrap();
        for f in ["rrn", "phone", "email"] {
            assert!(out.get(f).is_some(), "정책이 참조하는 필드 {f} 이(가) 없습니다");
        }
    }

    #[tokio::test]
    async fn sales_only_sees_contracts_of_managed_customers() {
        let t = contracts_tool();
        let out = t.handler.call(&sales(&["CUST-1001"]), &args(json!({"keyword":""}))).await.unwrap();
        let list = out["contracts"].as_array().unwrap();
        assert!(!list.is_empty());
        for c in list {
            assert_eq!(c["customer_id"], json!("CUST-1001"));
        }
    }

    #[tokio::test]
    async fn invisible_contracts_do_not_exist_rather_than_being_refused() {
        // 담당이 아닌 고객의 계약은 건수조차 새어나가면 안 됩니다.
        let t = contracts_tool();
        let out = t.handler.call(&sales(&["CUST-9999"]), &args(json!({"keyword":""}))).await.unwrap();
        assert!(out["contracts"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn finance_sees_all_contracts() {
        let t = contracts_tool();
        let finance = Identity {
            user_id: "emp-fin-01".into(),
            roles: vec!["finance".into()],
            ..Default::default()
        };
        let out = t.handler.call(&finance, &args(json!({"keyword":""}))).await.unwrap();
        assert_eq!(out["contracts"].as_array().unwrap().len(), 5);
    }

    #[tokio::test]
    async fn keyword_narrows_the_search() {
        let t = contracts_tool();
        let out = t
            .handler
            .call(&sales(&["CUST-1001", "CUST-1002"]), &args(json!({"keyword":"클라우드"})))
            .await
            .unwrap();
        let list = out["contracts"].as_array().unwrap();
        assert!(list.iter().all(|c| c["title"].as_str().unwrap().contains("클라우드")));
    }
}
