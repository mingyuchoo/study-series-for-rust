//! 헬프데스크 티켓 어댑터.
//!
//! `create_support_ticket` 은 **L3(승인 필요)** 입니다 — 티켓 생성은 되돌릴 수
//! 있지만 고객에게 나가는 행동이므로 사람이 한 번 봅니다. **재시도하지
//! 않습니다**(티켓이 두 번 생성되므로) — 레지스트리가 쓰기 도구의 `max_retries
//! > 0` 을 애초에 거부합니다.

use crate::legacy::{MemoryTransport,
                    Operation,
                    Transport,
                    not_found};
use ai_bridge::{adapter::{Adapter,
                          object,
                          str_enum,
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
use std::{collections::HashMap,
          sync::{Arc,
                 Mutex},
          time::Duration};

fn ticket_schema() -> Value {
    object(
        vec![
            ("ticket_id", str_prop("티켓 번호")),
            ("status", str_prop("상태 (open | in_progress | resolved | closed)")),
            ("subject", str_prop("제목")),
            ("customer_id", str_prop("고객 ID")),
            ("priority", str_prop("우선순위")),
            ("assignee", str_prop("담당자")),
        ],
        &["ticket_id", "status"],
    )
}

/// 프로세스 메모리에 사는 티켓 원장.
#[derive(Debug, Default)]
struct Store {
    tickets: Mutex<HashMap<String, Value>>,
    next: Mutex<u32>,
}

pub struct TicketAdapter {
    transport: Arc<dyn Transport>,
}

impl TicketAdapter {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self {
            transport,
        }
    }

    pub fn in_memory() -> Self {
        let store = Arc::new(Store::default());
        {
            let mut t = store.tickets.lock().unwrap();
            for v in mock_tickets() {
                t.insert(v["ticket_id"].as_str().unwrap().to_string(), v);
            }
            *store.next.lock().unwrap() = 5000;
        }
        let s = store.clone();
        Self::new(Arc::new(MemoryTransport::new(
            "ticket",
            Arc::new(move |op: Operation| {
                let s = s.clone();
                async move { memory_backend(&s, &op).await }
            }),
        )))
    }
}

async fn memory_backend(store: &Store, op: &Operation) -> Result<Value> {
    match op.name.as_str() {
        | "get_ticket" => {
            let id = op.path.last().cloned().unwrap_or_default();
            store.tickets.lock().unwrap().get(&id).cloned().ok_or_else(|| not_found(format!("티켓 {id}")))
        },
        | "create_ticket" => {
            let mut next = store.next.lock().unwrap();
            *next += 1;
            let id = format!("TKT-{}", *next);
            drop(next);

            let ticket = json!({
                "ticket_id": id,
                "status": "open",
                "subject": op.params.get("subject").cloned().unwrap_or(json!("")),
                "customer_id": op.params.get("customer_id").cloned().unwrap_or(json!("")),
                "priority": op.params.get("priority").cloned().unwrap_or(json!("normal")),
                "assignee": "",
            });
            store.tickets.lock().unwrap().insert(id.clone(), ticket.clone());
            Ok(ticket)
        },
        | other => Err(anyhow::anyhow!("ticket: unsupported operation {other:?}")),
    }
}

fn mock_tickets() -> Vec<Value> {
    vec![
        json!({"ticket_id":"TK-5001","status":"open","subject":"결제 오류 문의",
               "customer_id":"CUST-1001","priority":"high","assignee":"emp-support-01"}),
        json!({"ticket_id":"TKT-4001","status":"in_progress","subject":"로그인 오류",
               "customer_id":"CUST-1001","priority":"high","assignee":"emp-support-01"}),
        json!({"ticket_id":"TKT-4002","status":"resolved","subject":"청구서 문의",
               "customer_id":"CUST-1002","priority":"normal","assignee":"emp-support-02"}),
    ]
}

#[async_trait::async_trait]
impl Adapter for TicketAdapter {
    fn name(&self) -> String { "ticket".into() }

    async fn health_check(&self) -> Result<()> { self.transport.health().await }

    fn tools(&self) -> Vec<Tool> {
        let t1 = self.transport.clone();
        let t2 = self.transport.clone();

        vec![
            Tool {
                spec: Spec {
                    name: "get_ticket_status".into(),
                    description: "티켓 상태를 조회합니다.".into(),
                    system: "ticket".into(),
                    access: Access::Read,
                    risk_level: RiskLevel::L1,
                    sensitivity: Sensitivity::Internal,
                    required_permissions: vec!["ticket.read".into()],
                    rate_limit_per_min: 60,
                    timeout_ms: 5_000,
                    max_retries: 2,
                    log_retention_days: 90,
                    fallback: "고객지원팀(helpdesk@example.com)에 문의하세요.".into(),
                    input_schema: object(vec![("ticket_id", str_prop("티켓 번호. 예: TKT-4001"))], &["ticket_id"]),
                    output_schema: ticket_schema(),
                    ..Default::default()
                },
                handler: handler(move |_id: Identity, args: Map<String, Value>| {
                    let t = t1.clone();
                    async move {
                        let tid = args.get("ticket_id").and_then(|v| v.as_str()).unwrap_or_default();
                        t.call(&Operation::read("get_ticket").path(&["tickets", tid])).await
                    }
                }),
            },
            Tool {
                spec: Spec {
                    name: "create_support_ticket".into(),
                    description: "고객 지원 티켓을 생성합니다. 승인이 필요합니다.".into(),
                    system: "ticket".into(),
                    access: Access::Write,
                    // L3 — 되돌릴 수 있지만 고객에게 나가는 행동입니다.
                    risk_level: RiskLevel::L3,
                    sensitivity: Sensitivity::Internal,
                    required_permissions: vec!["ticket.write".into()],
                    // 되돌릴 수 있으므로 4시간.
                    approval_ttl: Duration::from_secs(4 * 3600),
                    rate_limit_per_min: 10,
                    timeout_ms: 5_000,
                    // **쓰기는 재시도하지 않습니다** — 티켓이 두 번 생성됩니다.
                    max_retries: 0,
                    log_retention_days: 365,
                    fallback: "고객지원팀(helpdesk@example.com)에 직접 요청하세요.".into(),
                    input_schema: object(
                        vec![
                            ("customer_id", str_prop("고객 ID")),
                            ("subject", str_prop("티켓 제목")),
                            ("priority", str_enum("우선순위", &["low", "normal", "high", "urgent"])),
                        ],
                        &["customer_id", "subject"],
                    ),
                    output_schema: ticket_schema(),
                    ..Default::default()
                },
                handler: handler(move |_id: Identity, args: Map<String, Value>| {
                    let t = t2.clone();
                    async move {
                        let mut op = Operation::write("create_ticket").path(&["tickets"]);
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

    #[tokio::test]
    async fn creates_a_ticket() {
        let a = TicketAdapter::in_memory();
        let t = a.tools().into_iter().find(|t| t.spec.name == "create_support_ticket").unwrap();
        let out = t
            .handler
            .call(
                &Identity::default(),
                &args(json!({"customer_id":"CUST-1001","subject":"결제 실패","priority":"high"})),
            )
            .await
            .unwrap();
        assert_eq!(out["status"], json!("open"));
        assert!(out["ticket_id"].as_str().unwrap().starts_with("TKT-"));
    }

    #[test]
    fn the_write_tool_is_l3_and_never_retries() {
        let a = TicketAdapter::in_memory();
        let t = a.tools().into_iter().find(|t| t.spec.name == "create_support_ticket").unwrap();
        assert_eq!(t.spec.risk_level, RiskLevel::L3);
        assert_eq!(t.spec.access, Access::Write);
        assert_eq!(t.spec.max_retries, 0, "쓰기 재시도는 중복 실행입니다");
        assert_eq!(t.spec.approval_ttl, Duration::from_secs(4 * 3600));
    }
}
