//! 환불 어댑터 — 다단계 업무 흐름 (L4).
//!
//! 환불은 단일 API 호출이 아니라 **업무 흐름**입니다. 이 순서를 LLM 에게 맡기면
//! 조건 확인을 건너뛰고 환불을 집행하는 일이 생깁니다. 그래서 게이트웨이는
//! **도구 하나만 노출하고, 그 안에서 워크플로 엔진이 순서와 보상을
//! 통제합니다.**
//!
//! ```text
//! process_refund(invoice_id, reason)
//!   1. lookup_invoice      송장 조회
//!   2. check_refundable    결제 완료 + 30일 이내인가
//!   3. calculate_amount    환불 금액 계산
//!   4. create_draft        환불 초안 생성       ← 보상: 미집행 초안 삭제
//!   5. execute_refund      환불 집행           ← 보상: reversed 로 되돌림
//!   6. notify_customer     고객 알림           ← 보상: 알림 회수
//! ```
//!
//! run ID 를 송장 ID 에서 **결정적으로** 만들기 때문에(`refund-INV-2026-0001`),
//! 환불 집행 직후 게이트웨이가 죽어도 같은 송장으로 다시 호출하면 완료된 단계를
//! 건너뛰고 재개합니다. 이미 끝난 흐름을 다시 호출하면 **돈이 두 번 나가지
//! 않고** 저장된 결과만 돌아옵니다.
//!
//! **승인은 이 엔진이 아니라 게이트웨이의 승인 관문이 담당합니다.**
//! `process_refund` 는 L4 이므로 승인 없이는 핸들러가 호출되지 않고, 엔진이
//! 도는 시점에는 이미 승인이 끝나 있습니다. 승인 관문을 두 군데 두면 어느 쪽이
//! 진짜인지 알 수 없게 됩니다.

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
                           handler},
                workflow::{self,
                           Definition,
                           Engine,
                           RetryPolicy,
                           State,
                           Step,
                           step_fn}};
use anyhow::{Result,
             anyhow};
use serde_json::{Map,
                 Value,
                 json};
use std::{collections::HashMap,
          sync::{Arc,
                 Mutex},
          time::Duration};

/// 환불 원장 — 무엇이 집행되고 무엇이 되돌려졌는지.
#[derive(Debug, Default)]
pub struct Ledger {
    /// 초안 (아직 돈이 나가지 않음).
    drafts: Mutex<HashMap<String, Value>>,
    /// 집행된 환불. `reversed` 로 표시될 수 있습니다.
    executed: Mutex<HashMap<String, Value>>,
    /// 보낸 알림.
    notifications: Mutex<Vec<String>>,
}

impl Ledger {
    pub fn executed(&self, run_id: &str) -> Option<Value> { self.executed.lock().unwrap().get(run_id).cloned() }

    pub fn notifications(&self) -> Vec<String> { self.notifications.lock().unwrap().clone() }
}

pub struct RefundAdapter {
    engine: Arc<Engine>,
    ledger: Arc<Ledger>,
    /// 송장 조회용 — 실제로는 ERP 어댑터를 부르지만, 참조 구현에서는 같은 목
    /// 데이터를 씁니다.
    invoices: Arc<Vec<Value>>,
}

impl RefundAdapter {
    pub fn new(store: Arc<dyn workflow::Store>) -> Self {
        Self {
            engine: Arc::new(Engine::new(store)),
            ledger: Arc::new(Ledger::default()),
            invoices: Arc::new(mock_invoices()),
        }
    }

    pub fn ledger(&self) -> Arc<Ledger> { self.ledger.clone() }

    /// 환불 흐름 정의.
    fn definition(&self) -> Definition {
        let invoices = self.invoices.clone();
        let ledger_draft = self.ledger.clone();
        let ledger_draft_c = self.ledger.clone();
        let ledger_exec = self.ledger.clone();
        let ledger_exec_c = self.ledger.clone();
        let ledger_notify = self.ledger.clone();
        let ledger_notify_c = self.ledger.clone();

        let plain = |name: &str, run: Arc<dyn workflow::StepFn>| Step {
            name: name.to_string(),
            run,
            compensate: None,
            timeout: Duration::from_secs(5),
            retry: RetryPolicy::default(),
        };

        Definition {
            name: "refund".into(),
            version: "1".into(),
            steps: vec![
                // 1. 송장 조회.
                plain(
                    "lookup_invoice",
                    step_fn(move |mut s: State| {
                        let invoices = invoices.clone();
                        async move {
                            let id = s.string("invoice_id");
                            let inv = invoices
                                .iter()
                                .find(|i| i["invoice_id"] == json!(id))
                                .cloned()
                                .ok_or_else(|| anyhow!("송장 {id} 을(를) 찾을 수 없습니다"))?;
                            s.set("status", inv["status"].clone());
                            s.set("amount", inv["amount"].clone());
                            s.set("paid_at", inv["paid_at"].clone());
                            s.set("customer_id", inv["customer_id"].clone());
                            Ok(s)
                        }
                    }),
                ),
                // 2. 환불 가능한가 — **LLM 이 건너뛸 수 없는 검사.**
                plain(
                    "check_refundable",
                    step_fn(move |s: State| async move {
                        if s.string("status") != "paid" {
                            return Err(anyhow!("결제 완료된 송장만 환불할 수 있습니다(현재: {})", s.string("status")));
                        }
                        let paid_at = s.string("paid_at");
                        let paid = ai_bridge::clock::parse_rfc3339(&format!("{paid_at}T00:00:00Z")).ok_or_else(|| anyhow!("결제일을 알 수 없습니다"))?;
                        let age = chrono::Utc::now() - paid;
                        if age > chrono::Duration::days(30) {
                            return Err(anyhow!("환불 가능 기간(30일)이 지났습니다(경과 {}일)", age.num_days()));
                        }
                        Ok(s)
                    }),
                ),
                // 3. 환불 금액 계산.
                plain(
                    "calculate_amount",
                    step_fn(move |mut s: State| async move {
                        let amount = s.int("amount");
                        s.set("refund_amount", json!(amount));
                        Ok(s)
                    }),
                ),
                // 4. 초안 생성 — 보상: 미집행 초안 삭제.
                Step {
                    name: "create_draft".into(),
                    run: step_fn(move |mut s: State| {
                        let l = ledger_draft.clone();
                        async move {
                            let draft_id = format!("RF-DRAFT-{}", s.string("invoice_id"));
                            l.drafts.lock().unwrap().insert(
                                draft_id.clone(),
                                json!({
                                    "draft_id": draft_id,
                                    "invoice_id": s.string("invoice_id"),
                                    "amount": s.int("refund_amount"),
                                }),
                            );
                            s.set("draft_id", json!(draft_id));
                            Ok(s)
                        }
                    }),
                    compensate: Some(step_fn(move |s: State| {
                        let l = ledger_draft_c.clone();
                        async move {
                            // 아직 돈이 나가지 않았으므로 그냥 지웁니다.
                            l.drafts.lock().unwrap().remove(&s.string("draft_id"));
                            Ok(s)
                        }
                    })),
                    timeout: Duration::from_secs(5),
                    retry: RetryPolicy::default(),
                },
                // 5. 환불 집행 — 보상: reversed 로 되돌림.
                Step {
                    name: "execute_refund".into(),
                    run: step_fn(move |mut s: State| {
                        let l = ledger_exec.clone();
                        async move {
                            let entry = json!({
                                "invoice_id": s.string("invoice_id"),
                                "amount": s.int("refund_amount"),
                                "status": "executed",
                                // **멱등 키.** 외부 시스템도 이 값을 저장·검사해야
                                // end-to-end 중복 방지가 완성됩니다.
                                "idempotency_key": s.activity_key.clone(),
                                "fencing_token": s.fencing_token,
                            });
                            l.executed.lock().unwrap().insert(s.run_id.clone(), entry);
                            s.set("executed", json!(true));
                            Ok(s)
                        }
                    }),
                    compensate: Some(step_fn(move |s: State| {
                        let l = ledger_exec_c.clone();
                        async move {
                            // **원장에서 기록을 지우지 않고 `reversed` 로 표시합니다.**
                            // 돈이 움직인 사실은 지울 수 없고, 되돌렸다는 사실도 기록입니다.
                            if let Some(e) = l.executed.lock().unwrap().get_mut(&s.run_id) {
                                e["status"] = json!("reversed");
                            }
                            Ok(s)
                        }
                    })),
                    timeout: Duration::from_secs(5),
                    retry: RetryPolicy::default(),
                },
                // 6. 고객 알림 — 보상: 알림 회수.
                Step {
                    name: "notify_customer".into(),
                    run: step_fn(move |mut s: State| {
                        let l = ledger_notify.clone();
                        async move {
                            let msg = format!(
                                "{} 님, 송장 {} 의 환불 {}원이 처리되었습니다.",
                                s.string("customer_id"),
                                s.string("invoice_id"),
                                s.int("refund_amount")
                            );
                            l.notifications.lock().unwrap().push(msg);
                            s.set("notified", json!(true));
                            Ok(s)
                        }
                    }),
                    compensate: Some(step_fn(move |s: State| {
                        let l = ledger_notify_c.clone();
                        async move {
                            l.notifications.lock().unwrap().pop();
                            Ok(s)
                        }
                    })),
                    timeout: Duration::from_secs(5),
                    retry: RetryPolicy::default(),
                },
            ],
        }
    }
}

fn mock_invoices() -> Vec<Value> {
    // ERP 와 같은 데이터. paid_at 은 항상 최근이라 30일 규칙이 통과하도록 둡니다.
    let recent = (chrono::Utc::now() - chrono::Duration::days(5)).format("%Y-%m-%d").to_string();
    let old = (chrono::Utc::now() - chrono::Duration::days(200)).format("%Y-%m-%d").to_string();
    vec![
        json!({"invoice_id":"INV-2026-0001","customer_id":"CUST-1001","status":"paid",
               "amount":1_200_000,"paid_at":recent}),
        json!({"invoice_id":"INV-2026-0002","customer_id":"CUST-1001","status":"unpaid",
               "amount":450_000,"paid_at":""}),
        json!({"invoice_id":"INV-2026-0004","customer_id":"CUST-2001","status":"paid",
               "amount":780_000,"paid_at":old}),
    ]
}

/// run ID 는 송장 ID 에서 **결정적으로** 만듭니다 — 그래서 재개와 멱등이
/// 가능합니다.
fn run_id_for(invoice_id: &str) -> String { format!("refund-{invoice_id}") }

#[async_trait::async_trait]
impl Adapter for RefundAdapter {
    fn name(&self) -> String { "refund".into() }

    async fn health_check(&self) -> Result<()> { Ok(()) }

    fn tools(&self) -> Vec<Tool> {
        let engine1 = self.engine.clone();
        let engine2 = self.engine.clone();
        let this = Arc::new(RefundAdapter {
            engine: self.engine.clone(),
            ledger: self.ledger.clone(),
            invoices: self.invoices.clone(),
        });

        vec![
            Tool {
                spec: Spec {
                    name: "get_workflow_status".into(),
                    description: "업무 흐름 실행 상태를 조회합니다.".into(),
                    system: "refund".into(),
                    access: Access::Read,
                    risk_level: RiskLevel::L1,
                    sensitivity: Sensitivity::Internal,
                    required_permissions: vec!["workflow.read".into()],
                    rate_limit_per_min: 60,
                    timeout_ms: 5_000,
                    max_retries: 2,
                    log_retention_days: 365,
                    fallback: "재무팀(finance-help@example.com)에 문의하세요.".into(),
                    input_schema: object(vec![("run_id", str_prop("실행 ID. 예: refund-INV-2026-0001"))], &["run_id"]),
                    output_schema: object(
                        vec![
                            ("run_id", str_prop("실행 ID")),
                            ("status", str_prop("상태")),
                            ("completed_steps", array_prop("완료된 단계", str_prop("단계명"))),
                            ("error", str_prop("오류")),
                        ],
                        &["run_id", "status"],
                    ),
                    ..Default::default()
                },
                handler: handler(move |_id: Identity, args: Map<String, Value>| {
                    let e = engine1.clone();
                    async move {
                        let run_id = args.get("run_id").and_then(|v| v.as_str()).unwrap_or_default();
                        let run = e.store().load(run_id).await?.ok_or_else(|| anyhow!("실행 {run_id} 을(를) 찾을 수 없습니다"))?;
                        Ok(json!({
                            "run_id": run.id,
                            "status": run.status.as_str(),
                            "completed_steps": run.completed,
                            "error": run.error,
                        }))
                    }
                }),
            },
            Tool {
                spec: Spec {
                    name: "process_refund".into(),
                    description: "환불을 처리합니다(6단계 업무 흐름). 승인이 필요하며 \
                                  기본적으로 차단되어 있습니다."
                        .into(),
                    system: "refund".into(),
                    access: Access::Write,
                    // **L4 — 자동 실행형.** 기본 차단(`-allow-high-risk` 필요) + 승인 필요.
                    risk_level: RiskLevel::L4,
                    sensitivity: Sensitivity::Confidential,
                    required_permissions: vec!["refund.execute".into()],
                    // 되돌리기 가장 어렵고 환불 가능 기간이 빨리 낡으므로 15분.
                    approval_ttl: Duration::from_secs(15 * 60),
                    rate_limit_per_min: 5,
                    timeout_ms: 30_000,
                    max_retries: 0,
                    log_retention_days: 365,
                    fallback: "재무팀(finance-help@example.com)에 환불을 요청하세요.".into(),
                    input_schema: object(
                        vec![("invoice_id", str_prop("송장 번호")), ("reason", str_prop("환불 사유"))],
                        &["invoice_id", "reason"],
                    ),
                    output_schema: object(
                        vec![
                            ("run_id", str_prop("실행 ID")),
                            ("status", str_prop("상태")),
                            ("refund_amount", int_prop("환불 금액")),
                            ("completed_steps", array_prop("완료된 단계", str_prop("단계명"))),
                        ],
                        &["run_id", "status"],
                    ),
                    ..Default::default()
                },
                handler: handler(move |_id: Identity, args: Map<String, Value>| {
                    let e = engine2.clone();
                    let this = this.clone();
                    async move {
                        let invoice_id = args.get("invoice_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                        let run_id = run_id_for(&invoice_id);

                        let mut input = Map::new();
                        input.insert("invoice_id".into(), json!(invoice_id));
                        input.insert("reason".into(), args.get("reason").cloned().unwrap_or(json!("")));

                        let def = this.definition();
                        let run = e.execute(&def, &run_id, Some(&input)).await?;

                        Ok(json!({
                            "run_id": run.id,
                            "status": run.status.as_str(),
                            "refund_amount": run.values.get("refund_amount").cloned().unwrap_or(json!(0)),
                            "completed_steps": run.completed,
                        }))
                    }
                }),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_bridge::workflow::MemoryStore;

    fn args(v: Value) -> Map<String, Value> { v.as_object().unwrap().clone() }

    fn adapter() -> RefundAdapter { RefundAdapter::new(Arc::new(MemoryStore::new())) }

    fn refund_tool(a: &RefundAdapter) -> Tool { a.tools().into_iter().find(|t| t.spec.name == "process_refund").unwrap() }

    #[test]
    fn process_refund_is_l4_with_a_short_approval_ttl() {
        let a = adapter();
        let t = refund_tool(&a);
        assert_eq!(t.spec.risk_level, RiskLevel::L4);
        assert_eq!(t.spec.access, Access::Write);
        assert_eq!(t.spec.max_retries, 0);
        // 되돌리기 가장 어려우므로 15분.
        assert_eq!(t.spec.approval_ttl, Duration::from_secs(15 * 60));
    }

    #[tokio::test]
    async fn refunds_a_paid_invoice_end_to_end() {
        let a = adapter();
        let ledger = a.ledger();
        let t = refund_tool(&a);

        let out = t
            .handler
            .call(&Identity::default(), &args(json!({"invoice_id":"INV-2026-0001","reason":"고객 요청"})))
            .await
            .unwrap();

        assert_eq!(out["status"], json!("completed"));
        assert_eq!(out["run_id"], json!("refund-INV-2026-0001"));
        assert_eq!(out["refund_amount"], json!(1_200_000));
        assert_eq!(out["completed_steps"].as_array().unwrap().len(), 6);

        let entry = ledger.executed("refund-INV-2026-0001").unwrap();
        assert_eq!(entry["status"], json!("executed"));
        // 외부 시스템에 넘어간 멱등 키.
        assert_eq!(entry["idempotency_key"], json!("refund-INV-2026-0001:recovery-0:execute_refund"));
        assert_eq!(ledger.notifications().len(), 1);
    }

    #[tokio::test]
    async fn calling_again_does_not_pay_twice() {
        // **멱등.** 이것이 지켜지지 않으면 돈이 두 번 나갑니다.
        let a = adapter();
        let ledger = a.ledger();
        let t = refund_tool(&a);
        let input = args(json!({"invoice_id":"INV-2026-0001","reason":"고객 요청"}));

        t.handler.call(&Identity::default(), &input).await.unwrap();
        let out = t.handler.call(&Identity::default(), &input).await.unwrap();

        assert_eq!(out["status"], json!("completed"));
        assert_eq!(ledger.notifications().len(), 1, "이미 끝난 흐름을 다시 호출했더니 알림이 두 번 나갔습니다");
    }

    #[tokio::test]
    async fn an_unpaid_invoice_is_refused_before_any_money_moves() {
        // LLM 이 순서를 건너뛰고 환불을 집행할 수 없습니다.
        let a = adapter();
        let ledger = a.ledger();
        let t = refund_tool(&a);

        let err = t
            .handler
            .call(&Identity::default(), &args(json!({"invoice_id":"INV-2026-0002","reason":"고객 요청"})))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("결제 완료된 송장만"));
        assert!(ledger.executed("refund-INV-2026-0002").is_none());
        assert!(ledger.notifications().is_empty());
    }

    #[tokio::test]
    async fn an_invoice_past_the_refund_window_is_refused() {
        let a = adapter();
        let t = refund_tool(&a);
        let err = t
            .handler
            .call(&Identity::default(), &args(json!({"invoice_id":"INV-2026-0004","reason":"고객 요청"})))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("환불 가능 기간"));
    }

    #[tokio::test]
    async fn workflow_status_is_queryable() {
        let a = adapter();
        let refund = refund_tool(&a);
        let status = a.tools().into_iter().find(|t| t.spec.name == "get_workflow_status").unwrap();

        refund
            .handler
            .call(&Identity::default(), &args(json!({"invoice_id":"INV-2026-0001","reason":"r"})))
            .await
            .unwrap();

        let out = status
            .handler
            .call(&Identity::default(), &args(json!({"run_id":"refund-INV-2026-0001"})))
            .await
            .unwrap();
        assert_eq!(out["status"], json!("completed"));
        assert_eq!(out["completed_steps"].as_array().unwrap().len(), 6);
    }
}
