//! 다단계 업무 흐름 엔진 — 지속 · 재개 · 보상 · lease.
//!
//! 환불은 단일 API 호출이 아니라 업무 흐름입니다. 이 순서를 LLM에게 맡기면 조건
//! 확인을 건너뛰고 환불을 집행하는 일이 생깁니다. 그래서 게이트웨이는 **도구
//! 하나만 노출하고, 그 안에서 이 엔진이 순서와 보상을 통제합니다.**
//!
//! **지속과 재개.** 단계 하나가 끝날 때마다 실행 상태를 저장합니다. 환불 집행
//! 직후 게이트웨이가 죽어도, 같은 송장으로 다시 호출하면 완료된 단계를 건너뛰고
//! 재개합니다.
//!
//! **보상 트랜잭션.** 알림 전송이 실패하면 이미 집행된 환불을 되돌립니다. 돈만
//! 나가고 고객은 모르는 상태로 두는 것보다 낫기 때문입니다. 원장에서 기록을
//! 지우지는 않고 `reversed` 로 표시합니다 — 돈이 움직인 사실은 지울 수 없고,
//! 되돌렸다는 사실도 기록입니다.
//!
//! **승인은 이 엔진이 아니라 게이트웨이의 승인 관문이 담당합니다.** 승인 관문을
//! 두 군데 두면 어느 쪽이 진짜인지 알 수 없게 됩니다.

mod engine;
mod memory;
mod postgres;
mod sqlite;

use anyhow::Result;
use chrono::{DateTime,
             Utc};
pub use engine::{Engine,
                 Scheduler};
pub use memory::MemoryStore;
pub use postgres::PostgresWorkflowStore;
use serde_json::{Map,
                 Value};
use sha2::{Digest,
           Sha256};
pub use sqlite::SqliteWorkflowStore;
use std::{collections::HashMap,
          time::Duration};

/// 워크플로 오류.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("workflow: run previously failed: {0}")]
    RunFailed(String),
    /// **낙관적 잠금 충돌.** 다른 인스턴스가 먼저 저장했습니다.
    #[error("workflow: version conflict (concurrent update)")]
    VersionConflict,
    /// 다른 worker 가 lease 를 쥐고 있습니다.
    #[error("workflow: run is leased by another worker")]
    LeaseHeld,
    #[error("workflow: run is waiting: {0}")]
    Waiting(String),
    /// 같은 run ID 를 다른 입력으로 재사용했습니다.
    #[error("workflow: run ID was reused with different input")]
    InputConflict,
    #[error("{0}")]
    Failed(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// 실행 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Running,
    Completed,
    /// 실패해서 **되돌렸음** — 보상이 성공한 상태.
    Compensated,
    /// 보상까지 실패 — **사람이 봐야 합니다.**
    Failed,
    /// 외부 이벤트를 기다리는 중. **종결 상태가 아닙니다.**
    Waiting,
    Cancelled,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            | Status::Running => "running",
            | Status::Completed => "completed",
            | Status::Compensated => "compensated",
            | Status::Failed => "failed",
            | Status::Waiting => "waiting",
            | Status::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            | "running" => Some(Status::Running),
            | "completed" => Some(Status::Completed),
            | "compensated" => Some(Status::Compensated),
            | "failed" => Some(Status::Failed),
            | "waiting" => Some(Status::Waiting),
            | "cancelled" => Some(Status::Cancelled),
            | _ => None,
        }
    }

    /// **`Waiting` 은 종결이 아닙니다** — 재개될 수 있습니다.
    pub fn terminal(self) -> bool { matches!(self, Status::Completed | Status::Compensated | Status::Failed | Status::Cancelled) }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.as_str()) }
}

/// 단계가 주고받는 상태.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub run_id: String,
    pub values: Map<String, Value>,
    pub step_name: String,
    /// **외부 시스템에 넘길 멱등 키.** `{run_id}:recovery-{n}:{step}` 형식.
    ///
    /// 외부 시스템도 이 값을 저장·검사해야 end-to-end 중복 방지가 완성됩니다.
    pub activity_key: String,
    /// 단조 증가하는 fencing token. 뒤늦게 깨어난 옛 worker 의 쓰기를 거부하는
    /// 데 씁니다.
    pub fencing_token: i64,
}

impl State {
    pub fn set(&mut self, key: &str, v: Value) { self.values.insert(key.to_string(), v); }

    pub fn string(&self, key: &str) -> String { self.values.get(key).and_then(|v| v.as_str()).unwrap_or_default().to_string() }

    /// JSON 왕복 후에는 정수가 f64 로 올 수 있으므로 둘 다 받습니다.
    pub fn int(&self, key: &str) -> i64 {
        match self.values.get(key) {
            | Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
            | _ => 0,
        }
    }
}

/// 단계를 실행하는 것.
#[async_trait::async_trait]
pub trait StepFn: Send + Sync {
    async fn call(&self, state: &mut State) -> Result<()>;
}

/// 클로저를 단계로 감쌉니다. 상태를 값으로 받아 바뀐 상태를 돌려줍니다.
pub fn step_fn<F, Fut>(f: F) -> std::sync::Arc<dyn StepFn>
where
    F: Fn(State) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<State>> + Send + 'static,
{
    struct W<F>(F);

    #[async_trait::async_trait]
    impl<F, Fut> StepFn for W<F>
    where
        F: Fn(State) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<State>> + Send,
    {
        async fn call(&self, state: &mut State) -> Result<()> {
            *state = (self.0)(state.clone()).await?;
            Ok(())
        }
    }

    std::sync::Arc::new(W(f))
}

/// 재시도 정책.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 1,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::ZERO,
        }
    }
}

/// 흐름의 한 단계.
pub struct Step {
    pub name: String,
    pub run: std::sync::Arc<dyn StepFn>,
    /// 되돌리는 방법. `None` 이면 되돌릴 것이 없습니다.
    pub compensate: Option<std::sync::Arc<dyn StepFn>>,
    pub timeout: Duration,
    pub retry: RetryPolicy,
}

impl std::fmt::Debug for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Step")
            .field("name", &self.name)
            .field("has_compensate", &self.compensate.is_some())
            .finish()
    }
}

/// 흐름 정의.
#[derive(Debug)]
pub struct Definition {
    pub name: String,
    pub version: String,
    pub steps: Vec<Step>,
}

/// 한 번의 실행.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Run {
    pub id: String,
    pub name: String,
    pub status: StatusOpt,
    /// 완료된 단계 이름 (실행 순서).
    pub completed: Vec<String>,
    pub values: Map<String, Value>,
    pub error: String,
    /// **보상까지 실패한 경우** — 사람이 확인해야 합니다.
    pub compensate_error: String,
    pub started_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    /// 낙관적 잠금. 새 run 은 0, 첫 저장 뒤 1.
    pub version: i64,
    pub definition_version: String,
    pub input_hash: String,
    pub current_step: String,
    pub lease_owner: String,
    pub lease_until: Option<DateTime<Utc>>,
    pub fencing_token: i64,
    pub next_run_at: Option<DateTime<Utc>>,
    /// `recover()` 가 증가시킵니다 — 멱등 키의 세대를 바꿉니다.
    pub recovery_count: i64,
}

/// `Run::status` 의 별칭.
pub type StatusOpt = Status;

/// append-only 이벤트.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub run_id: String,
    pub at: DateTime<Utc>,
    pub r#type: String,
    pub step: String,
    pub attempt: i64,
    pub worker: String,
    pub fencing_token: i64,
    pub message: String,
}

/// 엔진이 내는 이벤트 종류.
pub mod event_type {
    pub const WORKFLOW_STARTED: &str = "workflow_started";
    pub const STEP_STARTED: &str = "step_started";
    pub const STEP_COMPLETED: &str = "step_completed";
    pub const STEP_FAILED: &str = "step_failed";
    pub const WORKFLOW_WAITING: &str = "workflow_waiting";
    pub const COMPENSATION_STARTED: &str = "compensation_started";
    pub const COMPENSATION_COMPLETED: &str = "compensation_completed";
    pub const COMPENSATION_FAILED: &str = "compensation_failed";
    pub const WORKFLOW_RECOVERED: &str = "workflow_recovered";
    pub const WORKFLOW_CANCELLED: &str = "workflow_cancelled";
}

/// 워크플로 저장소.
///
/// # 교체 구현이 지켜야 할 것
///
/// **`save` 는 낙관적 잠금을 지켜야 합니다.** 같은 run ID 에 두 프로세스가
/// 동시에 Save 할 때 마지막 쓰기가 이기게 두면 안 됩니다 — 완료된 단계 목록이
/// **뒤로 돌아갈 수** 있습니다. `run.version` 이 저장소의 버전과 같을 때만
/// 갱신하고, 다르면 [`Error::VersionConflict`] 를 돌려주십시오.
#[async_trait::async_trait]
pub trait Store: Send + Sync {
    async fn load(&self, run_id: &str) -> Result<Option<Run>, Error>;
    /// 낙관적 잠금 upsert. 저장된 새 버전을 담아 돌려줍니다.
    async fn save(&self, run: &Run) -> Result<Run, Error>;
    async fn list(&self, status: Option<Status>, limit: i64) -> Result<Vec<Run>, Error>;
    async fn append_event(&self, e: &Event) -> Result<(), Error>;
    async fn events(&self, run_id: &str) -> Result<Vec<Event>, Error>;
}

/// 단계가 "지금은 못 하니 나중에 깨워달라"고 말하는 방법.
#[derive(Debug, Clone)]
pub struct WaitError {
    pub until: DateTime<Utc>,
    pub reason: String,
}

impl std::fmt::Display for WaitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "wait until {}: {}", crate::clock::to_rfc3339(self.until), self.reason) }
}

impl std::error::Error for WaitError {}

/// 흐름을 일시 정지시킵니다. **재시도되지 않고** 곧장 위로 올라갑니다.
pub fn wait_until(until: DateTime<Utc>, reason: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(WaitError {
        until,
        reason: reason.into(),
    })
}

/// 입력 해시 — 같은 run ID 를 다른 입력으로 재사용하는 것을 잡습니다.
pub fn hash_input(input: &Map<String, Value>) -> String {
    let bytes = serde_json::to_vec(&Value::Object(input.clone())).unwrap_or_default();
    let mut h = Sha256::new();
    h.update(&bytes);
    hex::encode(h.finalize())
}

// ---------------------------------------------------------------------------
// 메타데이터 사이드카
// ---------------------------------------------------------------------------

// 이 필드들은 **별도 컬럼이 아니라 `values_json` 안의 예약 키**로 저장됩니다.
// Go 판이 그렇게 하고 있고, 같은 DB 파일을 두 구현이 읽을 수 있어야 하므로
// 그대로 둡니다.
const META_DEFINITION_VERSION: &str = "_workflow_definition_version";
const META_INPUT_HASH: &str = "_workflow_input_hash";
const META_CURRENT_STEP: &str = "_workflow_current_step";
const META_LEASE_OWNER: &str = "_workflow_lease_owner";
const META_LEASE_UNTIL: &str = "_workflow_lease_until";
const META_FENCING_TOKEN: &str = "_workflow_fencing_token";
const META_NEXT_RUN_AT: &str = "_workflow_next_run_at";
const META_RECOVERY_COUNT: &str = "_workflow_recovery_count";

/// 저장 직전에 메타데이터를 `values` 안으로 밀어 넣습니다.
pub(crate) fn sync_metadata(run: &mut Run) {
    let v = &mut run.values;
    v.insert(META_DEFINITION_VERSION.into(), Value::String(run.definition_version.clone()));
    v.insert(META_INPUT_HASH.into(), Value::String(run.input_hash.clone()));
    v.insert(META_CURRENT_STEP.into(), Value::String(run.current_step.clone()));
    v.insert(META_LEASE_OWNER.into(), Value::String(run.lease_owner.clone()));
    v.insert(
        META_LEASE_UNTIL.into(),
        match run.lease_until {
            // lease/next_run_at 은 나노초 정밀도입니다.
            | Some(t) => Value::String(crate::clock::to_rfc3339_nanos(t)),
            | None => Value::String(String::new()),
        },
    );
    v.insert(META_FENCING_TOKEN.into(), Value::from(run.fencing_token));
    v.insert(
        META_NEXT_RUN_AT.into(),
        match run.next_run_at {
            | Some(t) => Value::String(crate::clock::to_rfc3339_nanos(t)),
            | None => Value::String(String::new()),
        },
    );
    v.insert(META_RECOVERY_COUNT.into(), Value::from(run.recovery_count));
}

/// 조회 직후에 메타데이터를 꺼냅니다.
pub(crate) fn hydrate_metadata(run: &mut Run) {
    let get_str = |v: &Map<String, Value>, k: &str| -> String { v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string() };
    let get_i64 = |v: &Map<String, Value>, k: &str| -> i64 { v.get(k).and_then(|x| x.as_i64()).unwrap_or(0) };
    let get_time = |v: &Map<String, Value>, k: &str| -> Option<DateTime<Utc>> {
        v.get(k)
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .and_then(crate::clock::parse_rfc3339)
    };

    run.definition_version = get_str(&run.values, META_DEFINITION_VERSION);
    run.input_hash = get_str(&run.values, META_INPUT_HASH);
    run.current_step = get_str(&run.values, META_CURRENT_STEP);
    run.lease_owner = get_str(&run.values, META_LEASE_OWNER);
    run.lease_until = get_time(&run.values, META_LEASE_UNTIL);
    run.fencing_token = get_i64(&run.values, META_FENCING_TOKEN);
    run.next_run_at = get_time(&run.values, META_NEXT_RUN_AT);
    run.recovery_count = get_i64(&run.values, META_RECOVERY_COUNT);
}

/// 테스트에서 lease 등 메타데이터를 미리 심을 때 씁니다.
pub fn sync_metadata_for_test(run: &mut Run) { sync_metadata(run); }

/// 멱등 키. **외부 Activity 가 저장·검사해야 중복 실행이 막힙니다.**
pub(crate) fn activity_key(run_id: &str, recovery_count: i64, step: &str) -> String { format!("{run_id}:recovery-{recovery_count}:{step}") }

/// 정의를 검증합니다.
pub(crate) fn validate_definition(def: &Definition, run_id: &str) -> Result<(), Error> {
    if run_id.is_empty() {
        return Err(Error::Other(anyhow::anyhow!("workflow: run ID is required")));
    }
    if def.name.is_empty() {
        return Err(Error::Other(anyhow::anyhow!("workflow: name is required")));
    }
    if def.steps.is_empty() {
        return Err(Error::Other(anyhow::anyhow!("workflow: at least one step is required")));
    }
    let mut seen = std::collections::HashSet::new();
    for s in &def.steps {
        if s.name.is_empty() {
            return Err(Error::Other(anyhow::anyhow!("workflow: step name is required")));
        }
        if !seen.insert(&s.name) {
            return Err(Error::Other(anyhow::anyhow!("workflow: duplicate step {:?}", s.name)));
        }
    }
    Ok(())
}

/// 저장소 구현이 쓰는 직렬화 헬퍼.
pub(crate) fn values_to_json(v: &Map<String, Value>) -> String { serde_json::to_string(&Value::Object(v.clone())).unwrap_or_else(|_| "{}".into()) }

pub(crate) fn values_from_json(s: &str) -> Map<String, Value> { serde_json::from_str::<Value>(s).ok().and_then(|v| v.as_object().cloned()).unwrap_or_default() }

pub(crate) fn completed_to_json(v: &[String]) -> String { serde_json::to_string(v).unwrap_or_else(|_| "[]".into()) }

pub(crate) fn completed_from_json(s: &str) -> Vec<String> { serde_json::from_str(s).unwrap_or_default() }

/// 워크플로 실행 목록 (운영 콘솔 · CLI).
pub type Runs = Vec<Run>;

/// 정의 이름과 버전으로 만든 스케줄러 키.
pub(crate) fn def_key(name: &str, version: &str) -> String { format!("{name}@{version}") }

/// 스케줄러가 관리하는 정의들.
pub(crate) type Definitions = HashMap<String, std::sync::Arc<Definition>>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn waiting_is_not_terminal() {
        // 대기 중인 흐름은 재개될 수 있으므로 종결이 아닙니다 — 취소도 가능해야 합니다.
        assert!(!Status::Waiting.terminal());
        assert!(!Status::Running.terminal());
        assert!(Status::Completed.terminal());
        assert!(Status::Failed.terminal());
        assert!(Status::Compensated.terminal());
        assert!(Status::Cancelled.terminal());
    }

    #[test]
    fn activity_key_encodes_the_recovery_generation() {
        // Recover 가 세대를 올리면 멱등 키가 달라져, 외부 시스템이 이전 시도와
        // 구분합니다.
        assert_eq!(activity_key("refund-INV-1", 0, "execute_refund"), "refund-INV-1:recovery-0:execute_refund");
        assert_eq!(activity_key("refund-INV-1", 1, "execute_refund"), "refund-INV-1:recovery-1:execute_refund");
    }

    #[test]
    fn input_hash_ignores_key_order() {
        let mut a = Map::new();
        a.insert("b".into(), json!(2));
        a.insert("a".into(), json!(1));
        let mut b = Map::new();
        b.insert("a".into(), json!(1));
        b.insert("b".into(), json!(2));
        assert_eq!(hash_input(&a), hash_input(&b));
    }

    #[test]
    fn metadata_round_trips_through_values() {
        let mut run = Run {
            id: "r1".into(),
            name: "refund".into(),
            definition_version: "1".into(),
            input_hash: "abc".into(),
            current_step: "execute".into(),
            lease_owner: "worker-1".into(),
            lease_until: Some(Utc::now()),
            fencing_token: 7,
            recovery_count: 2,
            ..Default::default()
        };
        sync_metadata(&mut run);

        let mut restored = Run {
            values: run.values.clone(),
            ..Default::default()
        };
        hydrate_metadata(&mut restored);

        assert_eq!(restored.definition_version, "1");
        assert_eq!(restored.input_hash, "abc");
        assert_eq!(restored.current_step, "execute");
        assert_eq!(restored.lease_owner, "worker-1");
        assert_eq!(restored.fencing_token, 7);
        assert_eq!(restored.recovery_count, 2);
        assert!(restored.lease_until.is_some());
    }

    #[test]
    fn state_int_survives_json_float_roundtrip() {
        let mut s = State::default();
        s.set("amount", json!(1_200_000.0));
        assert_eq!(s.int("amount"), 1_200_000);
        s.set("amount", json!(1_200_000));
        assert_eq!(s.int("amount"), 1_200_000);
    }
}
