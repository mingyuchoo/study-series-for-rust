//! 구매·발주 어댑터.
//!
//! **`draft_purchase_request`(L2) → `submit_purchase_request`(L3) 쌍이 등급의
//! 존재 이유를 보여줍니다.** LLM 은 초안을 승인 없이 몇 번이고 다듬을 수 있고,
//! 승인은 되돌리기 어려운 지점(제출)에만 걸립니다. 읽기/쓰기 2분법으로는 이
//! 구분이 불가능합니다.
//!
//! **금액은 제출 인자로 다시 받습니다.** 정책 엔진은 인자만 보고 판단하므로,
//! 금액이 인자에 없으면 "500만원 이상 발주 금지" 규칙이 발동할 수 없습니다.
//! 그리고 어댑터가 저장된 초안과 대조하므로 **LLM 이 금액을 낮춰 신고해 규칙을
//! 우회할 수 없습니다.**

use crate::legacy::{MemoryTransport,
                    Operation,
                    Transport,
                    not_found};
use ai_bridge::{adapter::{Adapter,
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
use anyhow::{Result,
             anyhow};
use serde_json::{Map,
                 Value,
                 json};
use std::{collections::HashMap,
          sync::{Arc,
                 Mutex},
          time::Duration};

#[derive(Debug, Default)]
struct Store {
    drafts: Mutex<HashMap<String, Value>>,
    submitted: Mutex<HashMap<String, Value>>,
    next: Mutex<u32>,
}

pub struct PurchaseAdapter {
    transport: Arc<dyn Transport>,
}

impl PurchaseAdapter {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self {
            transport,
        }
    }

    pub fn in_memory() -> Self {
        let store = Arc::new(Store::default());
        *store.next.lock().unwrap() = 700;
        let s = store.clone();
        Self::new(Arc::new(MemoryTransport::new(
            "purchase",
            Arc::new(move |op: Operation| {
                let s = s.clone();
                async move { memory_backend(&s, &op).await }
            }),
        )))
    }
}

/// 단가가 없으면 품목별 표준 단가를 씁니다.
fn unit_price_of(item: &str) -> i64 {
    match item {
        | "노트북" => 1_500_000,
        | "모니터" => 350_000,
        | "키보드" => 80_000,
        | _ => 100_000,
    }
}

async fn memory_backend(store: &Store, op: &Operation) -> Result<Value> {
    match op.name.as_str() {
        | "create_draft" => {
            let item = op.params.get("item").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let quantity = op.params.get("quantity").and_then(|v| v.as_i64()).unwrap_or(0);
            let unit_price = op.params.get("unit_price").and_then(|v| v.as_i64()).unwrap_or_else(|| unit_price_of(&item));
            let amount = unit_price * quantity;

            let mut next = store.next.lock().unwrap();
            *next += 1;
            let draft_id = format!("PR-DRAFT-{}", *next);
            drop(next);

            let draft = json!({
                "draft_id": draft_id,
                "item": item,
                "quantity": quantity,
                "unit_price": unit_price,
                "amount": amount,
                "status": "draft",
            });
            store.drafts.lock().unwrap().insert(draft_id.clone(), draft.clone());
            Ok(draft)
        },

        | "submit" => {
            let draft_id = op.params.get("draft_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let claimed = op.params.get("amount").and_then(|v| v.as_i64()).unwrap_or(-1);

            let draft = store
                .drafts
                .lock()
                .unwrap()
                .get(&draft_id)
                .cloned()
                .ok_or_else(|| not_found(format!("발주 초안 {draft_id}")))?;

            let real = draft["amount"].as_i64().unwrap_or(0);
            // **금액 대조.** 정책은 인자의 금액을 보고 판단했으므로, 그 금액이 초안과
            // 다르면 정책이 잘못된 근거로 판단한 것입니다 — 실행하면 안 됩니다.
            if claimed != real {
                return Err(anyhow!(
                    "발주 금액이 초안과 다릅니다(신고 {claimed}, 초안 {real}). \
                     초안 금액 그대로 제출하십시오."
                ));
            }

            let mut next = store.next.lock().unwrap();
            *next += 1;
            let request_id = format!("PR-{}", *next);
            drop(next);

            let submitted = json!({
                "request_id": request_id,
                "draft_id": draft_id,
                "item": draft["item"],
                "quantity": draft["quantity"],
                "amount": real,
                "status": "submitted",
            });
            store.submitted.lock().unwrap().insert(request_id.clone(), submitted.clone());
            Ok(submitted)
        },

        | other => Err(anyhow!("purchase: unsupported operation {other:?}")),
    }
}

#[async_trait::async_trait]
impl Adapter for PurchaseAdapter {
    fn name(&self) -> String { "purchase".into() }

    async fn health_check(&self) -> Result<()> { self.transport.health().await }

    fn tools(&self) -> Vec<Tool> {
        let t1 = self.transport.clone();
        let t2 = self.transport.clone();

        vec![
            Tool {
                spec: Spec {
                    name: "draft_purchase_request".into(),
                    description: "발주 초안을 작성합니다. 초안은 효력이 없으므로 승인이 \
                                  필요하지 않습니다."
                        .into(),
                    system: "purchase".into(),
                    access: Access::Write,
                    // **L2 — 쓰기지만 승인 불필요.** 초안은 아무것도 집행하지 않습니다.
                    risk_level: RiskLevel::L2,
                    sensitivity: Sensitivity::Internal,
                    required_permissions: vec!["purchase.draft".into()],
                    rate_limit_per_min: 20,
                    timeout_ms: 5_000,
                    max_retries: 0,
                    log_retention_days: 180,
                    fallback: "구매팀(procurement@example.com)에 문의하세요.".into(),
                    input_schema: object(
                        vec![
                            ("item", str_prop("품목명. 예: 노트북")),
                            ("quantity", int_prop("수량")),
                            ("unit_price", int_prop("단가(원). 생략 시 표준 단가")),
                            ("reason", str_prop("발주 사유 (선택)")),
                        ],
                        &["item", "quantity"],
                    ),
                    output_schema: object(
                        vec![
                            ("draft_id", str_prop("초안 번호")),
                            ("item", str_prop("품목명")),
                            ("quantity", int_prop("수량")),
                            ("unit_price", int_prop("단가")),
                            ("amount", int_prop("총액")),
                            ("status", str_prop("상태")),
                        ],
                        &["draft_id", "amount"],
                    ),
                    ..Default::default()
                },
                handler: handler(move |_id: Identity, args: Map<String, Value>| {
                    let t = t1.clone();
                    async move {
                        let mut op = Operation::write("create_draft").path(&["drafts"]);
                        op.params = args;
                        t.call(&op).await
                    }
                }),
            },
            Tool {
                spec: Spec {
                    name: "submit_purchase_request".into(),
                    description: "발주 초안을 제출합니다. 승인이 필요합니다.".into(),
                    system: "purchase".into(),
                    access: Access::Write,
                    // **L3 — 되돌리기 어려운 지점.** 여기에 승인이 걸립니다.
                    risk_level: RiskLevel::L3,
                    sensitivity: Sensitivity::Confidential,
                    required_permissions: vec!["purchase.submit".into()],
                    // 취소 절차가 있으므로 24시간.
                    approval_ttl: Duration::from_secs(24 * 3600),
                    rate_limit_per_min: 10,
                    timeout_ms: 5_000,
                    max_retries: 0,
                    log_retention_days: 365,
                    fallback: "구매팀(procurement@example.com)에 대면 결재를 요청하세요.".into(),
                    input_schema: object(
                        vec![
                            ("draft_id", str_prop("초안 번호")),
                            // 정책 엔진이 금액을 근거로 판단할 수 있도록 인자로 받습니다.
                            ("amount", int_prop("발주 총액(원). 초안 금액과 같아야 합니다")),
                        ],
                        &["draft_id", "amount"],
                    ),
                    output_schema: object(
                        vec![
                            ("request_id", str_prop("발주 번호")),
                            ("draft_id", str_prop("초안 번호")),
                            ("item", str_prop("품목명")),
                            ("quantity", int_prop("수량")),
                            ("amount", int_prop("총액")),
                            ("status", str_prop("상태")),
                        ],
                        &["request_id", "status"],
                    ),
                    ..Default::default()
                },
                handler: handler(move |_id: Identity, args: Map<String, Value>| {
                    let t = t2.clone();
                    async move {
                        let mut op = Operation::write("submit").path(&["requests"]);
                        op.params = args;
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

    fn args(v: Value) -> Map<String, Value> { v.as_object().unwrap().clone() }

    fn tools() -> (Tool, Tool) {
        let a = PurchaseAdapter::in_memory();
        let ts = a.tools();
        let draft = ts.iter().find(|t| t.spec.name == "draft_purchase_request").unwrap().clone();
        let submit = ts.iter().find(|t| t.spec.name == "submit_purchase_request").unwrap().clone();
        (draft, submit)
    }

    #[test]
    fn draft_is_l2_and_submit_is_l3() {
        // 이 경계가 등급의 존재 이유입니다 — 둘 다 쓰기지만 승인은 하나에만 걸립니다.
        let (draft, submit) = tools();
        assert_eq!(draft.spec.access, Access::Write);
        assert_eq!(draft.spec.risk_level, RiskLevel::L2);
        assert_eq!(submit.spec.risk_level, RiskLevel::L3);
        assert_eq!(submit.spec.approval_ttl, Duration::from_secs(24 * 3600));
    }

    #[tokio::test]
    async fn drafts_then_submits() {
        let (draft, submit) = tools();
        let d = draft
            .handler
            .call(&Identity::default(), &args(json!({"item":"노트북","quantity":2})))
            .await
            .unwrap();
        assert_eq!(d["amount"], json!(3_000_000));

        let out = submit
            .handler
            .call(&Identity::default(), &args(json!({"draft_id": d["draft_id"], "amount": 3_000_000})))
            .await
            .unwrap();
        assert_eq!(out["status"], json!("submitted"));
    }

    #[tokio::test]
    async fn understating_the_amount_to_dodge_the_policy_is_rejected() {
        // 정책은 인자의 금액으로 판단합니다. 어댑터가 초안과 대조하지 않으면
        // LLM 이 금액을 낮춰 신고해 "500만원 이상 금지" 규칙을 우회할 수 있습니다.
        let (draft, submit) = tools();
        let d = draft
            .handler
            .call(&Identity::default(), &args(json!({"item":"노트북","quantity":10})))
            .await
            .unwrap();
        assert_eq!(d["amount"], json!(15_000_000));

        let err = submit
            .handler
            .call(
                &Identity::default(),
                // 정책을 통과하려고 100만원이라고 신고합니다.
                &args(json!({"draft_id": d["draft_id"], "amount": 1_000_000})),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("초안과 다릅니다"));
    }

    #[tokio::test]
    async fn submitting_an_unknown_draft_is_a_business_error() {
        let (_, submit) = tools();
        let err = submit
            .handler
            .call(&Identity::default(), &args(json!({"draft_id":"PR-DRAFT-9999","amount":1})))
            .await
            .unwrap_err();
        assert!(!ai_bridge::transient::is_temporary(&err));
    }
}
