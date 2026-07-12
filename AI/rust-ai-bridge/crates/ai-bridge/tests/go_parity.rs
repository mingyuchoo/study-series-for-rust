//! **Go 판과의 해시 바이트 단위 동등성 검사.**
//!
//! 포팅 설계에서 "바이트 단위로 재현해야 하는 것"으로 꼽은 네 가지 해시가
//! 실제로 Go 판과 같은 값을 내는지 확인합니다. 아래 기대값은 `go-ai-bridge` 의
//! **실제 함수**로 계산한 것입니다(내부 일관성이 아니라 진짜 교차 검증):
//!
//! ```text
//! go test ./internal/audit/    -run TestCrossCheckHash      -v   # audit.integrityHash
//! go run  ./crosscheck_main.go                                   # approval.Fingerprint · eval.ComputeContentHash · eval.ArgsDigest
//! go test ./internal/workflow/ -run TestCrossCheckHashInput  -v  # workflow.hashInput
//! ```
//!
//! 이 값이 어긋나면 Go 가 쓴 감사 DB·승인 저장소를 Rust 게이트웨이가 읽을 수
//! 없고, 그 반대도 마찬가지입니다. serde 의 직렬화 순서(맵=사전순,
//! 구조체=선언순)가 Go 의 `encoding/json` 과 정확히 일치해야만 통과합니다.

use ai_bridge::{approval,
                audit::{self,
                        Entry},
                eval::{self,
                       ToolStep},
                workflow};
use chrono::TimeZone;
use serde_json::json;

fn obj(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> { v.as_object().unwrap().clone() }

/// 감사 해시 체인 — genesis(prev="")와 이어진 항목(prev="deadbeef").
#[test]
fn audit_integrity_hash_matches_go() {
    let e = Entry {
        id: 1,
        timestamp: chrono::Utc.with_ymd_and_hms(2026, 7, 10, 9, 0, 0).unwrap(),
        actor: "emp-sales-01".into(),
        tool: "get_invoice_status".into(),
        system: "erp".into(),
        access: "read".into(),
        decision: "allowed".into(),
        reason: "허용".into(),
        approval_status: "n/a".into(),
        approval_id: String::new(),
        request_id: "req-abc".into(),
        session_id: "sess-1".into(),
        masked: true,
        input: Some(obj(json!({"invoice_id": "INV-2026-0001", "amount": 1_200_000}))),
        output: None,
        latency_ms: 12,
        input_tokens: 1500,
        output_tokens: 200,
        cost_micros: 4200,
        error: String::new(),
        prompt: "INV-2026-0001 결제됐어?".into(),
        injection: "프롬프트: override_instructions".into(),
    };

    // go test ./internal/audit/ -run TestCrossCheckHash -v
    assert_eq!(
        audit::integrity_hash("", &e),
        "499a4ab91894fb4ca67ed13b2b69fee1dad1f921f136f6156e375f27ac56089c",
        "audit genesis 해시가 Go 와 다릅니다"
    );
    assert_eq!(
        audit::integrity_hash("deadbeef", &e),
        "76f65ff59101836e8c1a978437efa84faaa04961b1d385d890aeaedd4a1a3a1f",
        "audit chained 해시가 Go 와 다릅니다"
    );
}

/// 승인 지문 — (주체, 도구, 인자).
#[test]
fn approval_fingerprint_matches_go() {
    let args = obj(json!({"invoice_id": "INV-2026-0001", "amount": 5_000_000, "note": "재무 검토"}));
    assert_eq!(
        approval::fingerprint("emp-sales-01", "process_refund", &args),
        "da066cdf1f357bbbbfc85db59a5bc324114c6409a8252ebf3e04c2656add2a37",
        "승인 지문이 Go 와 다릅니다 — 관리자가 승인한 것과 실행되는 것이 어긋납니다"
    );
}

/// eval 콘텐츠 해시 — (prompt, reply, trail).
#[test]
fn eval_content_hash_matches_go() {
    let trail = vec![
        ToolStep {
            name: "get_invoice_status".into(),
            args_digest: "abc".into(),
            decision: "allowed".into(),
            error_code: String::new(),
            audit_request_id: "req-1".into(),
        },
        ToolStep {
            name: "process_refund".into(),
            args_digest: String::new(),
            decision: "denied".into(),
            error_code: "high_risk_blocked".into(),
            audit_request_id: String::new(),
        },
    ];
    assert_eq!(
        eval::compute_content_hash("환불해줘", "고위험이라 차단됐습니다.", &trail),
        "0363841407c628329858ef827deb900807d02f6f7d8432a0034292f4964d64a3",
        "eval 콘텐츠 해시가 Go 와 다릅니다"
    );
}

/// eval 인자 다이제스트.
#[test]
fn eval_args_digest_matches_go() {
    let args = obj(json!({"invoice_id": "INV-2026-0001", "amount": 5_000_000, "note": "재무 검토"}));
    assert_eq!(
        eval::args_digest(&args),
        "7e0c5ba6d47c1a8f6c2c5026545aa0aaf4c865949a1034392dc45e0069b67acc",
        "eval 인자 다이제스트가 Go 와 다릅니다"
    );
}

/// 워크플로 입력 해시 — run ID 재사용 검사의 근거.
#[test]
fn workflow_input_hash_matches_go() {
    let input = obj(json!({"invoice_id": "INV-2026-0001", "reason": "고객 요청", "amount": 1_200_000}));
    assert_eq!(
        workflow::hash_input(&input),
        "1adf6d9a11faf6b859d433019370bf9b4e95d9eab7f201c8935d28604e570910",
        "워크플로 입력 해시가 Go 와 다릅니다"
    );
}
