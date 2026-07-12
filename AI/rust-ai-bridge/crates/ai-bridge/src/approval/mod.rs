//! 승인 저장소 — 지문 기반 단회 승인.
//!
//! L3 이상 도구는 **승인 없이는 절대 실행되지 않습니다.** 승인은 (주체, 도구,
//! 인자)의 지문에 묶이므로, 관리자가 승인한 내용과 실제 실행되는 내용이 항상
//! 일치합니다. 인자가 한 글자라도 다르면 지문이 달라져 기존 승인이 적용되지
//! 않습니다.
//!
//! **승인은 단회성입니다.** 한 번 실행에 소비되면 같은 호출이라도 다시 승인을
//! 받아야 합니다.
//!
//! **시계는 관리자가 결정한 시점부터 흐릅니다** — 요청이 만들어진 시점이
//! 아닙니다. 관리자가 승인한 것은 "이 호출을 해도 된다"가 아니라 **"지금 이
//! 상황에서 이 호출을 해도 된다"** 이기 때문입니다. 예산·재고·고객 상태 같은
//! 근거는 시간이 지나면 사라집니다.
//!
//! # 교체 구현이 지켜야 할 것
//!
//! - [`Store::ensure`] — "승인된 요청을 찾아 소비한다"가 **원자적**이어야
//!   합니다. 두 프로세스가 같은 승인을 동시에 소비하면 도구가 두 번 실행됩니다.
//! - [`Store::decide`] — 요청자·결정자 대조가 상태 전이와 **같은 트랜잭션
//!   안**에 있어야 합니다. 밖에서 검사하면 검사와 갱신 사이에 다른 결정이
//!   끼어듭니다.

mod postgres;
mod sqlite;

use anyhow::Result;
use chrono::{DateTime,
             Utc};
pub use postgres::PostgresApprovalStore;
use serde_json::{Map,
                 Value};
use sha2::{Digest,
           Sha256};
pub use sqlite::SqliteApprovalStore;
use std::time::Duration;

/// 지정하지 않은 도구의 기본 유효 기간. **무기한 유효한 승인은 있을 수
/// 없습니다.**
pub const DEFAULT_TTL: Duration = Duration::from_secs(3600);

/// 승인 요청 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pending,
    Approved,
    Rejected,
    /// 실행에 소비됨 — 다시 쓸 수 없습니다.
    Consumed,
    Expired,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            | Status::Pending => "pending",
            | Status::Approved => "approved",
            | Status::Rejected => "rejected",
            | Status::Consumed => "consumed",
            | Status::Expired => "expired",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            | "pending" => Some(Status::Pending),
            | "approved" => Some(Status::Approved),
            | "rejected" => Some(Status::Rejected),
            | "consumed" => Some(Status::Consumed),
            | "expired" => Some(Status::Expired),
            | _ => None,
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.as_str()) }
}

/// 승인 저장소 오류. 호출자가 종류를 구분해야 하므로 열거형입니다.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("approval: request not found")]
    NotFound,
    #[error("approval: request is not pending")]
    NotPending,
    #[error("approval: request state changed concurrently")]
    Raced,
    #[error("approval: requester cannot decide their own request: {0:?}")]
    SelfApproval(String),
    #[error("approval: approver identity is required")]
    NoApprover,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// 승인 요청.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub id: String,
    pub fingerprint: String,
    pub actor: String,
    pub tool: String,
    pub args: Map<String, Value>,
    pub status: Status,
    pub requested_at: DateTime<Utc>,
    pub decided_by: String,
    pub decided_at: Option<DateTime<Utc>>,
    pub note: String,
    pub ttl: Duration,
    /// 승인되기 전에는 `None` — 결정 시점부터 시계가 흐릅니다.
    pub expires_at: Option<DateTime<Utc>>,
}

impl Request {
    /// 만료 경계는 **배타적**입니다 — 정확히 만료 시각인 순간은 아직
    /// 유효합니다.
    pub fn expired(&self, now: DateTime<Utc>) -> bool { self.expires_at.map(|e| now > e).unwrap_or(false) }
}

/// 승인 저장소.
#[async_trait::async_trait]
pub trait Store: Send + Sync {
    /// 이 호출에 대한 승인 상태를 확정합니다.
    ///
    /// - 유효한 승인이 있으면 **원자적으로 소비하고** `Approved` 를 돌려줍니다.
    /// - 승인됐지만 만료됐으면 `expired` 로 표시하고 **새 대기 요청**을 만들어
    ///   돌려줍니다.
    /// - 대기 중이면 그대로 돌려줍니다 (같은 요청을 중복 생성하지 않습니다).
    /// - 거부됐으면 그대로 돌려줍니다 (인자를 바꾸면 지문이 달라져 새 요청이
    ///   됩니다).
    /// - 아무것도 없으면 새 대기 요청을 만듭니다.
    async fn ensure(&self, actor: &str, tool: &str, args: &Map<String, Value>, ttl: Duration) -> Result<Request, Error>;

    /// 승인하거나 거부합니다. **요청자는 자기 요청을 결정할 수 없습니다.**
    async fn decide(&self, id: &str, approve: bool, by: &str, note: &str) -> Result<Request, Error>;

    async fn get(&self, id: &str) -> Result<Request, Error>;

    /// `status` 가 `None` 이면 전체.
    async fn list(&self, status: Option<Status>, limit: i64) -> Result<Vec<Request>, Error>;
}

/// (주체, 도구, 인자)의 지문.
///
/// 인자 정규화는 **키 사전순 정렬**이 곧 canonical form 입니다 —
/// `serde_json::Map` 이 BTreeMap 이므로 삽입 순서와 무관하게 같은 바이트가
/// 나옵니다. 이것이 "관리자가 승인한 내용과 실제 실행되는 내용이 같다"를
/// 보장합니다.
pub fn fingerprint(actor: &str, tool: &str, args: &Map<String, Value>) -> String {
    let args_json = serde_json::to_vec(&Value::Object(args.clone())).unwrap_or_else(|_| b"{}".to_vec());
    let mut h = Sha256::new();
    h.update(actor.as_bytes());
    h.update([0u8]);
    h.update(tool.as_bytes());
    h.update([0u8]);
    h.update(&args_json);
    hex::encode(h.finalize())
}

/// 새 요청 ID (`req_` + 12 hex).
pub(crate) fn new_id() -> String {
    use rand::Rng as _;
    let mut b = [0u8; 6];
    rand::rng().fill_bytes(&mut b);
    format!("req_{}", hex::encode(b))
}

/// TTL 이 0 이면 기본값을 씁니다.
pub(crate) fn effective_ttl(ttl: Duration) -> Duration { if ttl.is_zero() { DEFAULT_TTL } else { ttl } }

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn args(v: Value) -> Map<String, Value> { v.as_object().unwrap().clone() }

    #[test]
    fn fingerprint_is_stable() {
        let a = args(json!({"invoice_id": "INV-1", "amount": 100}));
        assert_eq!(fingerprint("emp-1", "process_refund", &a), fingerprint("emp-1", "process_refund", &a));
    }

    #[test]
    fn fingerprint_ignores_arg_insertion_order() {
        // 키 정렬이 canonical form 입니다.
        let mut m1 = Map::new();
        m1.insert("b".into(), json!(2));
        m1.insert("a".into(), json!(1));
        let mut m2 = Map::new();
        m2.insert("a".into(), json!(1));
        m2.insert("b".into(), json!(2));
        assert_eq!(fingerprint("x", "t", &m1), fingerprint("x", "t", &m2));
    }

    #[test]
    fn fingerprint_binds_to_actor_tool_and_args() {
        let a = args(json!({"amount": 100}));
        let base = fingerprint("emp-1", "t", &a);
        assert_ne!(fingerprint("emp-2", "t", &a), base);
        assert_ne!(fingerprint("emp-1", "other", &a), base);
        // 인자가 한 글자만 달라도 지문이 달라집니다.
        assert_ne!(fingerprint("emp-1", "t", &args(json!({"amount": 101}))), base);
    }

    #[test]
    fn expiry_boundary_is_exclusive() {
        let now = Utc::now();
        let r = Request {
            id: "req_1".into(),
            fingerprint: String::new(),
            actor: String::new(),
            tool: String::new(),
            args: Map::new(),
            status: Status::Approved,
            requested_at: now,
            decided_by: String::new(),
            decided_at: Some(now),
            note: String::new(),
            ttl: Duration::from_secs(60),
            expires_at: Some(now + chrono::Duration::seconds(60)),
        };
        // 정확히 만료 시각인 순간은 아직 유효합니다.
        assert!(!r.expired(now + chrono::Duration::seconds(60)));
        assert!(r.expired(now + chrono::Duration::seconds(61)));
    }

    #[test]
    fn unapproved_request_never_expires() {
        let r = Request {
            id: "req_1".into(),
            fingerprint: String::new(),
            actor: String::new(),
            tool: String::new(),
            args: Map::new(),
            status: Status::Pending,
            requested_at: Utc::now(),
            decided_by: String::new(),
            decided_at: None,
            note: String::new(),
            ttl: DEFAULT_TTL,
            expires_at: None,
        };
        assert!(!r.expired(Utc::now() + chrono::Duration::days(365)));
    }

    #[test]
    fn default_ttl_is_applied_to_zero() {
        assert_eq!(effective_ttl(Duration::ZERO), DEFAULT_TTL);
        assert_eq!(effective_ttl(Duration::from_secs(900)), Duration::from_secs(900));
    }

    #[test]
    fn ids_are_unique_and_prefixed() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
        assert!(a.starts_with("req_"));
        assert_eq!(a.len(), 4 + 12);
    }
}
