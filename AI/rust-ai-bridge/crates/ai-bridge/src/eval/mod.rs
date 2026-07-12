//! 품질 평가 — 턴 스냅샷 · 레이팅 · 골든셋.
//!
//! **평가는 게이트웨이 집행 경로에 영향을 주지 않습니다.** 관리자 재라벨도 자동
//! 점수도 append-only 입니다 — 사람 피드백(`human_user`)을 자동 채점이 덮어쓰면
//! 무엇이 진짜 신호인지 알 수 없게 됩니다.

pub mod golden;
pub mod judge;
mod sqlite;

use anyhow::Result;
use chrono::{DateTime,
             Utc};
use serde::{Deserialize,
            Serialize};
use serde_json::{Map,
                 Value};
use sha2::{Digest,
           Sha256};
pub use sqlite::SqliteEvalStore;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("eval: not found")]
    NotFound,
    #[error("eval: invalid input: {0}")]
    Invalid(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// 점수의 출처. **사람과 자동 채점을 섞지 않습니다.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// 최종 사용자의 👍/👎.
    HumanUser,
    /// 관리자 재라벨.
    HumanReviewer,
    /// 규칙 기반 자동 채점.
    AutoRubric,
    /// LLM judge 자동 채점.
    AutoLlmJudge,
    /// 골든셋 회귀.
    Golden,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            | Source::HumanUser => "human_user",
            | Source::HumanReviewer => "human_reviewer",
            | Source::AutoRubric => "auto_rubric",
            | Source::AutoLlmJudge => "auto_llm_judge",
            | Source::Golden => "golden",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            | "human_user" => Some(Source::HumanUser),
            | "human_reviewer" => Some(Source::HumanReviewer),
            | "auto_rubric" => Some(Source::AutoRubric),
            | "auto_llm_judge" => Some(Source::AutoLlmJudge),
            | "golden" => Some(Source::Golden),
            | _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    Thumbs,
    Likert5,
    BinaryPass,
}

impl Scale {
    pub fn as_str(self) -> &'static str {
        match self {
            | Scale::Thumbs => "thumbs",
            | Scale::Likert5 => "likert5",
            | Scale::BinaryPass => "binary_pass",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            | "thumbs" => Some(Scale::Thumbs),
            | "likert5" => Some(Scale::Likert5),
            | "binary_pass" => Some(Scale::BinaryPass),
            | _ => None,
        }
    }
}

/// 무엇이 잘못됐는지 — 집계해서 개선 방향을 봅니다.
pub const KNOWN_LABELS: &[&str] = &[
    "wrong_fact",
    "missing_citation",
    "missed_tool",
    "wrong_tool",
    "over_refusal",
    "policy_blind",
    "pii_leak",
    "hallucination",
    "incomplete",
    "other",
];

pub fn is_known_label(l: &str) -> bool { KNOWN_LABELS.contains(&l) }

/// 👍 = 1.0, 👎 = 0.0.
pub fn normalize_thumbs(up: bool) -> f64 { if up { 1.0 } else { 0.0 } }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Completed,
    Halted,
    MaxTurns,
    ModelRefusal,
    Error,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            | Outcome::Completed => "completed",
            | Outcome::Halted => "halted",
            | Outcome::MaxTurns => "max_turns",
            | Outcome::ModelRefusal => "model_refusal",
            | Outcome::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            | "completed" => Some(Outcome::Completed),
            | "halted" => Some(Outcome::Halted),
            | "max_turns" => Some(Outcome::MaxTurns),
            | "model_refusal" => Some(Outcome::ModelRefusal),
            | "error" => Some(Outcome::Error),
            | _ => None,
        }
    }
}

/// 도구 호출 한 건의 흔적.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolStep {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub args_digest: String,
    /// `allowed` | `denied` | `dry_run`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub decision: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error_code: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub audit_request_id: String,
}

/// 한 턴의 스냅샷. **불변입니다** — 같은 turn_id 로 다시 쓰면 오류입니다.
#[derive(Debug, Clone, Default)]
pub struct Turn {
    pub turn_id: String,
    pub session_id: String,
    pub actor: String,
    pub agent_id: String,
    pub timestamp: Option<DateTime<Utc>>,
    /// `chat` | `cli` | `mcp-external` | `api` | `golden` | `""`(모름).
    pub channel: String,
    pub model: String,
    pub prompt: String,
    pub reply: String,
    pub tool_trail: Vec<ToolStep>,
    pub outcome: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_micros: i64,
    pub content_hash: String,
}

/// 점수 한 건. **append-only** — 여러 출처의 점수가 서로 덮어쓰지 않고
/// 공존합니다.
#[derive(Debug, Clone, Default)]
pub struct Rating {
    pub id: i64,
    pub turn_id: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub source: String,
    pub rater_id: String,
    /// 0.0 ~ 1.0.
    pub score: f64,
    pub scale: String,
    pub labels: Vec<String>,
    pub note: String,
    pub rubric_id: String,
    pub rubric_version: String,
    pub dims: Map<String, Value>,
}

/// 골든셋 배치 실행.
#[derive(Debug, Clone, Default)]
pub struct Run {
    pub run_id: String,
    pub suite: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub git_sha: String,
    pub model: String,
    pub pass_count: i64,
    pub fail_count: i64,
}

#[derive(Debug, Clone, Default)]
pub struct RunResult {
    pub id: i64,
    pub run_id: String,
    pub case_id: String,
    pub pass: bool,
    pub score: f64,
    pub actual_reply: String,
    pub trail: Vec<ToolStep>,
    pub error: String,
}

#[derive(Debug, Clone, Default)]
pub struct TurnFilter {
    pub actor: String,
    pub agent_id: String,
    pub session_id: String,
    pub channel: String,
    pub outcome: String,
    pub unrated_only: bool,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub limit: i64,
}

#[derive(Debug, Clone, Default)]
pub struct RatingFilter {
    pub turn_id: String,
    pub source: String,
    pub scale: String,
    pub rater_id: String,
    pub label: String,
    pub since: Option<DateTime<Utc>>,
    pub limit: i64,
}

#[derive(Debug, Clone, Default)]
pub struct StatFilter {
    pub source: String,
    pub scale: String,
    pub since: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    Agent,
    Channel,
    Source,
    Scale,
    Label,
}

#[derive(Debug, Clone, Default)]
pub struct Stat {
    pub key: String,
    pub turns: i64,
    pub ratings: i64,
    pub avg_score: f64,
    pub thumbs_up: i64,
    pub thumbs_down: i64,
}

/// 평가 저장소.
#[async_trait::async_trait]
pub trait Store: Send + Sync {
    async fn record_turn(&self, t: &Turn) -> Result<Turn, Error>;
    async fn rate(&self, r: &Rating) -> Result<Rating, Error>;
    async fn get_turn(&self, turn_id: &str) -> Result<Turn, Error>;
    async fn query_turns(&self, f: &TurnFilter) -> Result<Vec<Turn>, Error>;
    async fn query_ratings(&self, f: &RatingFilter) -> Result<Vec<Rating>, Error>;
    async fn stats(&self, by: GroupBy, f: &StatFilter) -> Result<Vec<Stat>, Error>;

    async fn record_run(&self, r: &Run) -> Result<(), Error>;
    async fn add_run_result(&self, r: &RunResult) -> Result<(), Error>;
    async fn finish_run(&self, run_id: &str, finished_at: DateTime<Utc>, pass: i64, fail: i64) -> Result<(), Error>;
    async fn list_runs(&self, limit: i64) -> Result<Vec<Run>, Error>;
    async fn get_run_results(&self, run_id: &str) -> Result<Vec<RunResult>, Error>;
}

/// 턴 내용의 해시 — 같은 대화가 다시 기록됐는지 알아보는 데 씁니다.
pub fn compute_content_hash(prompt: &str, reply: &str, trail: &[ToolStep]) -> String {
    #[derive(Serialize)]
    struct Payload<'a> {
        prompt: &'a str,
        reply: &'a str,
        trail: &'a [ToolStep],
    }
    let bytes = serde_json::to_vec(&Payload {
        prompt,
        reply,
        trail,
    })
    .unwrap_or_default();
    let mut h = Sha256::new();
    h.update(&bytes);
    hex::encode(h.finalize())
}

/// 인자의 해시 — 원문을 남기지 않고 "같은 인자였는지"만 비교합니다.
pub fn args_digest(args: &Map<String, Value>) -> String {
    if args.is_empty() {
        return String::new();
    }
    let bytes = serde_json::to_vec(&Value::Object(args.clone())).unwrap_or_default();
    let mut h = Sha256::new();
    h.update(&bytes);
    hex::encode(h.finalize())
}

pub(crate) fn new_id(prefix: &str) -> String {
    use rand::Rng as _;
    let mut b = [0u8; 8];
    rand::rng().fill_bytes(&mut b);
    format!("{prefix}_{}", hex::encode(b))
}

pub(crate) fn validate_turn(t: &Turn) -> Result<(), Error> {
    if t.prompt.is_empty() && t.reply.is_empty() && t.tool_trail.is_empty() {
        return Err(Error::Invalid("turn has no prompt, reply, or tool trail".into()));
    }
    if !t.outcome.is_empty() && Outcome::parse(&t.outcome).is_none() {
        return Err(Error::Invalid(format!("unknown outcome {:?}", t.outcome)));
    }
    Ok(())
}

pub(crate) fn validate_rating(r: &Rating) -> Result<(), Error> {
    if r.turn_id.is_empty() {
        return Err(Error::Invalid("turn_id is required".into()));
    }
    if Source::parse(&r.source).is_none() {
        return Err(Error::Invalid(format!("unknown source {:?}", r.source)));
    }
    if Scale::parse(&r.scale).is_none() {
        return Err(Error::Invalid(format!("unknown scale {:?}", r.scale)));
    }
    if r.rater_id.is_empty() {
        return Err(Error::Invalid("rater_id is required".into()));
    }
    if !(0.0 ..= 1.0).contains(&r.score) {
        return Err(Error::Invalid(format!("score {} out of [0,1]", r.score)));
    }
    for l in &r.labels {
        if !is_known_label(l) {
            return Err(Error::Invalid(format!("unknown label {l:?}")));
        }
    }
    for (k, v) in &r.dims {
        let n = v.as_f64().unwrap_or(-1.0);
        if !(0.0 ..= 1.0).contains(&n) {
            return Err(Error::Invalid(format!("dim {k:?} out of [0,1]")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn content_hash_is_stable_and_sensitive() {
        let trail = vec![ToolStep {
            name: "get_invoice_status".into(),
            decision: "allowed".into(),
            ..Default::default()
        }];
        let a = compute_content_hash("q", "a", &trail);
        assert_eq!(a, compute_content_hash("q", "a", &trail));
        assert_ne!(a, compute_content_hash("q", "different", &trail));
        assert_ne!(a, compute_content_hash("q", "a", &[]));
    }

    #[test]
    fn args_digest_ignores_key_order() {
        let mut m1 = Map::new();
        m1.insert("b".into(), json!(2));
        m1.insert("a".into(), json!(1));
        let mut m2 = Map::new();
        m2.insert("a".into(), json!(1));
        m2.insert("b".into(), json!(2));
        assert_eq!(args_digest(&m1), args_digest(&m2));
        assert_eq!(args_digest(&Map::new()), "");
    }

    #[test]
    fn rating_validation_rejects_bad_values() {
        let ok = Rating {
            turn_id: "t1".into(),
            source: "human_user".into(),
            scale: "thumbs".into(),
            rater_id: "u1".into(),
            score: 1.0,
            ..Default::default()
        };
        assert!(validate_rating(&ok).is_ok());

        let mut bad = ok.clone();
        bad.score = 1.5;
        assert!(validate_rating(&bad).is_err());

        let mut bad = ok.clone();
        bad.source = "made_up".into();
        assert!(validate_rating(&bad).is_err());

        let mut bad = ok.clone();
        bad.labels = vec!["not_a_label".into()];
        assert!(validate_rating(&bad).is_err());

        let mut bad = ok;
        bad.rater_id = String::new();
        assert!(validate_rating(&bad).is_err());
    }

    #[test]
    fn empty_turn_is_rejected() {
        assert!(validate_turn(&Turn::default()).is_err());
        let t = Turn {
            prompt: "hi".into(),
            ..Default::default()
        };
        assert!(validate_turn(&t).is_ok());
    }

    #[test]
    fn thumbs_normalize() {
        assert_eq!(normalize_thumbs(true), 1.0);
        assert_eq!(normalize_thumbs(false), 0.0);
    }
}
