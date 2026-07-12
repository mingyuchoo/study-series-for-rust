//! 감사 로그 — append-only, 해시 체인, 보존 기간.
//!
//! 기록은 **덧붙이기만** 합니다. `UPDATE` 를 노출하지 않으며, 각 행은 이전
//! 해시와 묶인 **해시 체인**으로 연결됩니다. 행 삭제·수정·체인 단절은
//! [`SqliteLogger::verify_integrity`] 로 검출합니다.
//!
//! 게이트웨이는 [`Recorder`](쓰기 전용)만 받습니다 — 파이프라인이 감사 기록을
//! 조회하거나 지울 수 있게 두지 않습니다. 콘솔은 [`Reader`], `auditctl` 만
//! [`Purger`] 를 씁니다.

mod archive;
mod postgres;
mod retention;
mod sqlite;

use anyhow::Result;
pub use archive::{Discard,
                  Exporter,
                  FailingExporter,
                  FileExporter,
                  RecordingExporter,
                  SyslogExporter,
                  build_exporter};
use chrono::{DateTime,
             Utc};
pub use postgres::PostgresLogger;
pub use retention::{Policy,
                    Purged,
                    cutoff};
use serde::Serialize;
use serde_json::{Map,
                 Value};
use sha2::{Digest,
           Sha256};
pub use sqlite::SqliteLogger;
use std::collections::HashMap;

/// 감사 기록 한 건.
///
/// **필드 순서가 해시 입력을 결정합니다** — [`integrity_hash`] 참고. 순서를
/// 바꾸면 기존 체인이 전부 깨집니다.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Entry {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub actor: String,
    pub tool: String,
    pub system: String,
    pub access: String,
    /// `allowed` | `denied`.
    pub decision: String,
    pub reason: String,
    /// `n/a` | `pending` | `approved` | `rejected`.
    pub approval_status: String,
    pub approval_id: String,
    pub request_id: String,
    pub session_id: String,
    pub masked: bool,
    pub input: Option<Map<String, Value>>,
    pub output: Option<Map<String, Value>>,
    pub latency_ms: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    /// 비용(마이크로 단위). 부동소수 오차를 피하려고 정수로 둡니다.
    pub cost_micros: i64,
    pub error: String,
    pub prompt: String,
    /// 인젝션 신호. 예: `"프롬프트: override_instructions"`. 비면 신호 없음.
    pub injection: String,
}

/// Go 의 `encoding/json` 이 `Entry` 를 직렬화하는 모습을 그대로 재현합니다.
///
/// Go 는 구조체 필드를 **선언 순서대로**, 맵 키를 **사전순**으로 냅니다. 필드
/// 이름은 태그가 없으므로 Go 의 대문자 필드명 그대로입니다. `serde` 의 기본
/// 동작(선언 순서 + `serde_json::Map` 의 BTreeMap 정렬)이 이와 일치하므로,
/// `preserve_order` 피처를 켜면 **안 됩니다.**
#[derive(Serialize)]
#[allow(non_snake_case)]
struct HashEntry<'a> {
    ID: i64,
    Timestamp: String,
    Actor: &'a str,
    Tool: &'a str,
    System: &'a str,
    Access: &'a str,
    Decision: &'a str,
    Reason: &'a str,
    ApprovalStatus: &'a str,
    ApprovalID: &'a str,
    RequestID: &'a str,
    SessionID: &'a str,
    Masked: bool,
    Input: &'a Option<Map<String, Value>>,
    Output: &'a Option<Map<String, Value>>,
    LatencyMS: i64,
    InputTokens: i64,
    OutputTokens: i64,
    CostMicros: i64,
    Error: &'a str,
    Prompt: &'a str,
    Injection: &'a str,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct HashInput<'a> {
    Prev: &'a str,
    Entry: HashEntry<'a>,
}

/// 체인 해시: `sha256(json({Prev, Entry}))`.
///
/// 첫 항목의 `prev` 는 빈 문자열입니다.
pub fn integrity_hash(prev: &str, e: &Entry) -> String {
    let payload = HashInput {
        Prev: prev,
        Entry: HashEntry {
            ID: e.id,
            // 저장 형식과 해시 입력을 같게 정규화합니다 (초 정밀도 RFC3339).
            Timestamp: crate::clock::to_rfc3339(e.timestamp),
            Actor: &e.actor,
            Tool: &e.tool,
            System: &e.system,
            Access: &e.access,
            Decision: &e.decision,
            Reason: &e.reason,
            ApprovalStatus: &e.approval_status,
            ApprovalID: &e.approval_id,
            RequestID: &e.request_id,
            SessionID: &e.session_id,
            Masked: e.masked,
            Input: &e.input,
            Output: &e.output,
            LatencyMS: e.latency_ms,
            InputTokens: e.input_tokens,
            OutputTokens: e.output_tokens,
            CostMicros: e.cost_micros,
            Error: &e.error,
            Prompt: &e.prompt,
            Injection: &e.injection,
        },
    };
    let bytes = serde_json::to_vec(&payload).expect("Entry is always JSON-serializable");
    let mut h = Sha256::new();
    h.update(&bytes);
    hex::encode(h.finalize())
}

/// 조회 필터.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub actor: String,
    pub tool: String,
    pub system: String,
    pub session_id: String,
    /// `allowed` | `denied`.
    pub decision: String,
    pub errors_only: bool,
    pub masked_only: bool,
    pub injection_only: bool,
    pub since: Option<DateTime<Utc>>,
    pub limit: i64,
}

/// 집계 축. **SQL 컬럼명 그대로**입니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    Actor,
    Tool,
    System,
    Session,
}

impl GroupBy {
    /// SQL 컬럼명. 문자열 보간에 쓰이므로 **열거형 밖의 값이 들어올 수 없어야**
    /// 합니다.
    pub fn column(self) -> &'static str {
        match self {
            | GroupBy::Actor => "actor",
            | GroupBy::Tool => "tool",
            | GroupBy::System => "system",
            | GroupBy::Session => "session_id",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            | "actor" => Some(GroupBy::Actor),
            | "tool" => Some(GroupBy::Tool),
            | "system" => Some(GroupBy::System),
            | "session" | "session_id" => Some(GroupBy::Session),
            | _ => None,
        }
    }
}

/// 집계 결과.
#[derive(Debug, Clone, Default)]
pub struct Stat {
    pub key: String,
    pub calls: i64,
    pub denied: i64,
    pub errors: i64,
    pub avg_latency_ms: f64,
    pub max_latency_ms: i64,
    pub cost_micros: i64,
}

/// 기록만 합니다. **게이트웨이가 받는 것은 이것뿐입니다.**
#[async_trait::async_trait]
pub trait Recorder: Send + Sync {
    async fn log(&self, e: &Entry) -> Result<()>;
}

/// 읽기만 합니다. 운영 콘솔이 씁니다.
#[async_trait::async_trait]
pub trait Reader: Send + Sync {
    async fn query(&self, f: &Filter) -> Result<Vec<Entry>>;
    async fn recent(&self, limit: i64) -> Result<Vec<Entry>>;
    async fn stats(&self, by: GroupBy, since: Option<DateTime<Utc>>) -> Result<Vec<Stat>>;
    /// 도구별 가장 오래된 기록 시각.
    async fn oldest(&self) -> Result<HashMap<String, DateTime<Utc>>>;
}

/// 지웁니다. `auditctl` 만 씁니다.
#[async_trait::async_trait]
pub trait Purger: Send + Sync {
    async fn purge(&self, p: &Policy, now: DateTime<Utc>, exp: &dyn Exporter) -> Result<Purged>;
}

/// 셋 다.
pub trait Store: Recorder + Reader + Purger {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn entry() -> Entry {
        Entry {
            id: 1,
            timestamp: Utc.with_ymd_and_hms(2026, 7, 10, 9, 0, 0).unwrap(),
            actor: "emp-sales-01".into(),
            tool: "get_invoice_status".into(),
            system: "erp".into(),
            access: "read".into(),
            decision: "allowed".into(),
            input: Some(json!({"invoice_id": "INV-1"}).as_object().unwrap().clone()),
            ..Default::default()
        }
    }

    #[test]
    fn hash_is_stable_for_the_same_entry() {
        let e = entry();
        assert_eq!(integrity_hash("", &e), integrity_hash("", &e));
        assert_eq!(integrity_hash("", &e).len(), 64);
    }

    #[test]
    fn hash_changes_when_any_field_changes() {
        let e = entry();
        let base = integrity_hash("", &e);

        let mut tampered = e.clone();
        tampered.decision = "denied".into();
        assert_ne!(integrity_hash("", &tampered), base);

        let mut tampered = e.clone();
        tampered.actor = "someone-else".into();
        assert_ne!(integrity_hash("", &tampered), base);
    }

    #[test]
    fn hash_depends_on_the_previous_link() {
        // 이것이 체인을 만듭니다 — 앞 행을 지우면 뒤 행의 해시가 맞지 않게 됩니다.
        let e = entry();
        assert_ne!(integrity_hash("", &e), integrity_hash("abc", &e));
    }

    #[test]
    fn map_keys_are_sorted_not_insertion_ordered() {
        // Go 의 encoding/json 은 맵 키를 사전순으로 냅니다. serde_json 의 기본
        // BTreeMap 백엔드가 같은 순서를 내야 해시가 일치합니다.
        let mut a = Entry::default();
        let mut m1 = Map::new();
        m1.insert("b".into(), json!(1));
        m1.insert("a".into(), json!(2));
        a.input = Some(m1);

        let mut b = Entry::default();
        let mut m2 = Map::new();
        m2.insert("a".into(), json!(2));
        m2.insert("b".into(), json!(1));
        b.input = Some(m2);

        assert_eq!(integrity_hash("", &a), integrity_hash("", &b));
    }

    #[test]
    fn hash_input_uses_go_field_names_in_declaration_order() {
        // Go 구조체는 태그가 없으므로 대문자 필드명이 그대로 키가 되고, 선언 순서가
        // 유지됩니다. 직렬화 형태가 바뀌면 기존 체인을 검증할 수 없게 됩니다.
        let e = entry();
        let payload = HashInput {
            Prev: "",
            Entry: HashEntry {
                ID: e.id,
                Timestamp: crate::clock::to_rfc3339(e.timestamp),
                Actor: &e.actor,
                Tool: &e.tool,
                System: &e.system,
                Access: &e.access,
                Decision: &e.decision,
                Reason: &e.reason,
                ApprovalStatus: &e.approval_status,
                ApprovalID: &e.approval_id,
                RequestID: &e.request_id,
                SessionID: &e.session_id,
                Masked: e.masked,
                Input: &e.input,
                Output: &e.output,
                LatencyMS: e.latency_ms,
                InputTokens: e.input_tokens,
                OutputTokens: e.output_tokens,
                CostMicros: e.cost_micros,
                Error: &e.error,
                Prompt: &e.prompt,
                Injection: &e.injection,
            },
        };
        let s = serde_json::to_string(&payload).unwrap();
        assert!(s.starts_with(r#"{"Prev":"","Entry":{"ID":1,"Timestamp":"2026-07-10T09:00:00Z","Actor":"emp-sales-01""#));
        // nil 맵은 null 로 나갑니다 (Go 와 동일).
        assert!(s.contains(r#""Output":null"#));
    }

    #[test]
    fn group_by_columns_are_fixed() {
        assert_eq!(GroupBy::Session.column(), "session_id");
        assert_eq!(GroupBy::parse("session"), Some(GroupBy::Session));
        assert_eq!(GroupBy::parse("bogus"), None);
    }
}
