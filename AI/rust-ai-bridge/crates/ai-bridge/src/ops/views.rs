//! 조회 — 콘솔과 CLI 가 함께 쓰는 뷰 모델.

use super::{Error,
            Service};
use crate::{approval::{self,
                       Status},
            audit,
            breaker,
            eval,
            registry::Spec,
            workflow};
use anyhow::Result;
use chrono::{DateTime,
             Utc};
use serde_json::{Map,
                 Value};
use std::{collections::HashMap,
          time::Duration};

/// 도구 호출 한 건.
#[derive(Debug, Clone)]
pub struct CallRow {
    pub timestamp: DateTime<Utc>,
    pub actor: String,
    pub tool: String,
    pub system: String,
    pub access: String,
    pub decision: String,
    pub reason: String,
    pub approval_status: String,
    pub approval_id: String,
    pub request_id: String,
    pub session_id: String,
    pub masked: bool,
    pub latency_ms: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_micros: i64,
    pub error: String,
    pub prompt: String,
    pub injection: String,
}

impl From<audit::Entry> for CallRow {
    fn from(e: audit::Entry) -> Self {
        Self {
            timestamp: e.timestamp,
            actor: e.actor,
            tool: e.tool,
            system: e.system,
            access: e.access,
            decision: e.decision,
            reason: e.reason,
            approval_status: e.approval_status,
            approval_id: e.approval_id,
            request_id: e.request_id,
            session_id: e.session_id,
            masked: e.masked,
            latency_ms: e.latency_ms,
            input_tokens: e.input_tokens,
            output_tokens: e.output_tokens,
            cost_micros: e.cost_micros,
            error: e.error,
            prompt: e.prompt,
            injection: e.injection,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CallFilter {
    pub tool: String,
    pub actor: String,
    pub session_id: String,
    pub decision: String,
    pub errors_only: bool,
    pub masked_only: bool,
    pub injection_only: bool,
    pub since: Option<DateTime<Utc>>,
    pub limit: i64,
}

impl From<&CallFilter> for audit::Filter {
    fn from(f: &CallFilter) -> Self {
        audit::Filter {
            tool: f.tool.clone(),
            actor: f.actor.clone(),
            session_id: f.session_id.clone(),
            decision: f.decision.clone(),
            errors_only: f.errors_only,
            masked_only: f.masked_only,
            injection_only: f.injection_only,
            since: f.since,
            limit: f.limit,
            system: String::new(),
        }
    }
}

pub type CallStat = audit::Stat;

#[derive(Debug, Clone)]
pub struct ApprovalRow {
    pub id: String,
    pub actor: String,
    pub tool: String,
    pub status: String,
    pub requested_at: DateTime<Utc>,
    pub decided_by: String,
    pub ttl: Duration,
    pub expires_at: Option<DateTime<Utc>>,
    pub args: Map<String, Value>,
    /// 남은 시간 (사람이 읽는 문자열).
    pub remaining: String,
    /// **결정 버튼을 그릴지.** 요청자 본인에게는 그리지 않습니다.
    pub decidable: bool,
    /// 내가 요청한 것.
    pub own: bool,
}

/// **보상 실패는 사람이 봐야 합니다** — 그래서 뷰 모델이 그것을 실어 나릅니다.
#[derive(Debug, Clone)]
pub struct WorkflowRow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub completed: Vec<String>,
    pub updated_at: Option<DateTime<Utc>>,
    pub error: String,
    /// 비어 있지 않으면 **보상까지 실패한 것**입니다.
    pub compensate_error: String,
}

#[derive(Debug, Clone)]
pub struct HealthRow {
    pub system: String,
    pub healthy: bool,
    pub latency: Duration,
    pub impact: String,
    pub owner: String,
    pub contact: String,
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct BreakerRow {
    pub system: String,
    pub state: String,
    pub failures: i64,
    pub retry_in: Duration,
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub name: String,
    pub display_name: String,
    pub interface: String,
    pub capabilities: Vec<String>,
    pub data_sensitivity: Vec<String>,
    pub owner_team: String,
    pub contact: String,
    pub auth_method: String,
    pub failure_impact: String,
    pub realtime: String,
}

#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub system: String,
    pub access: String,
    pub risk_level: String,
    pub sensitivity: String,
    pub approval_ttl: Duration,
    pub log_retention_days: i64,
    pub required_permissions: Vec<String>,
}

impl From<Spec> for ToolInfo {
    fn from(s: Spec) -> Self {
        Self {
            name: s.name,
            system: s.system,
            access: s.access.to_string(),
            risk_level: s.risk_level.to_string(),
            sensitivity: s.sensitivity.to_string(),
            approval_ttl: s.approval_ttl,
            log_retention_days: s.log_retention_days,
            required_permissions: s.required_permissions,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetentionRow {
    pub tool: String,
    pub days: i64,
    pub oldest: Option<DateTime<Utc>>,
    /// 보존 기간을 넘겼음.
    pub over: bool,
}

#[derive(Debug, Clone)]
pub struct AgentRow {
    pub user_id: String,
    pub roles: Vec<String>,
    pub allowed_tools: Vec<String>,
    pub allowed_systems: Vec<String>,
    pub has_token: bool,
    pub expiry: String,
    pub token_sha256: String,
    pub token_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct AgentCatalog {
    pub roles: Vec<String>,
    pub tools: Vec<String>,
    pub systems: Vec<String>,
    pub agents: Vec<AgentRow>,
}

/// 24시간 대시보드.
#[derive(Debug, Clone, Default)]
pub struct Dashboard {
    pub calls: usize,
    pub denied: usize,
    pub errors: usize,
    pub masked: usize,
    /// 인젝션 신호가 걸린 호출.
    pub flagged: usize,
    pub pending: usize,
    pub unhealthy: usize,
    /// **보상 실패** — 사람이 손봐야 하는 흐름.
    pub failed_flows: usize,
    pub costly: Vec<crate::budget::Entry>,
    pub eval_enabled: bool,
    pub eval_turns: usize,
    pub eval_unrated: usize,
    pub eval_thumbs_rate: String,
}

const DAY: i64 = 24;

impl Service {
    // --- 감사 ---

    pub async fn query_calls(&self, f: &CallFilter) -> Result<Vec<CallRow>> {
        let entries = self.d.audit.query(&f.into()).await?;
        Ok(entries.into_iter().map(CallRow::from).collect())
    }

    pub async fn count_calls(&self, f: &CallFilter) -> Result<usize> { Ok(self.d.audit.query(&f.into()).await?.len()) }

    pub async fn call_stats(&self, by: &str, since: Option<DateTime<Utc>>) -> Result<Vec<CallStat>> {
        let axis = audit::GroupBy::parse(by).unwrap_or(audit::GroupBy::Tool);
        self.d.audit.stats(axis, since).await
    }

    // --- 승인 ---

    pub async fn list_approvals(&self, status: &str, viewer: &str, limit: i64) -> Result<Vec<ApprovalRow>, Error> {
        let filter = if status.is_empty() || status == "all" { None } else { Status::parse(status) };
        let limit = if limit <= 0 { 100 } else { limit };
        let reqs = self.d.approvals.list(filter, limit).await?;
        let now = chrono::Utc::now();

        Ok(reqs
            .into_iter()
            .map(|r| {
                let own = r.actor == viewer;
                ApprovalRow {
                    // **요청자 본인에게는 버튼을 그리지 않습니다.** 다만 그것은 편의일 뿐,
                    // 실제 차단은 승인 저장소가 합니다.
                    decidable: r.status == Status::Pending && !own,
                    own: own && r.status == Status::Pending,
                    remaining: remaining(&r, now),
                    id: r.id,
                    actor: r.actor,
                    tool: r.tool,
                    status: r.status.as_str().to_string(),
                    requested_at: r.requested_at,
                    decided_by: r.decided_by,
                    ttl: r.ttl,
                    expires_at: r.expires_at,
                    args: r.args,
                }
            })
            .collect())
    }

    pub async fn count_pending_approvals(&self) -> Result<usize, Error> { Ok(self.d.approvals.list(Some(Status::Pending), 1000).await?.len()) }

    /// 승인하거나 거부합니다.
    ///
    /// **결정자는 폼이 아니라 세션에서 옵니다.** 그리고 요청자 == 결정자 검사는
    /// 저장소가 합니다 — 콘솔이든 CLI 든 우회할 수 없습니다.
    pub async fn decide_approval(&self, id: &str, approve: bool, by: &str, note: &str) -> Result<(), Error> {
        self.d.approvals.decide(id, approve, by, note).await?;
        Ok(())
    }

    // --- 워크플로 ---

    pub async fn list_workflows(&self, status: &str, limit: i64) -> Result<Vec<WorkflowRow>> {
        let Some(store) = &self.d.workflows else {
            return Ok(Vec::new());
        };
        let filter = if status.is_empty() || status == "all" {
            None
        } else {
            workflow::Status::parse(status)
        };
        let limit = if limit <= 0 { 100 } else { limit };
        let runs = store.list(filter, limit).await.map_err(|e| anyhow::anyhow!("{e}"))?;

        Ok(runs
            .into_iter()
            .map(|r| WorkflowRow {
                id: r.id,
                name: r.name,
                status: r.status.as_str().to_string(),
                completed: r.completed,
                updated_at: r.updated_at,
                error: r.error,
                compensate_error: r.compensate_error,
            })
            .collect())
    }

    // --- 레거시 상태 ---

    pub async fn health(&self) -> Vec<HealthRow> {
        if self.d.adapters.is_empty() {
            return Vec::new();
        }
        let rows = crate::app::check_health(&self.d.adapters, Duration::from_secs(5)).await;
        rows.into_iter()
            .map(|h| {
                let sys = self.d.inventory.as_ref().and_then(|i| i.lookup(&h.system));
                HealthRow {
                    impact: sys.as_ref().map(|s| s.failure_impact.to_string()).unwrap_or_default(),
                    owner: sys.as_ref().map(|s| s.owner_team.clone()).unwrap_or_default(),
                    contact: sys.map(|s| s.contact).unwrap_or_default(),
                    system: h.system,
                    healthy: h.healthy,
                    latency: h.latency,
                    error: h.error,
                }
            })
            .collect()
    }

    pub fn breakers(&self) -> Vec<BreakerRow> {
        let Some(b) = &self.d.breakers else {
            return Vec::new();
        };
        b.statuses()
            .into_iter()
            .map(|s: breaker::Status| BreakerRow {
                system: s.key,
                state: s.state.to_string(),
                failures: s.failures,
                retry_in: s.retry_in,
            })
            .collect()
    }

    // --- 인벤토리 ---

    pub fn inventory(&self) -> (Vec<SystemInfo>, Vec<ToolInfo>, HashMap<String, usize>) {
        let systems: Vec<SystemInfo> = self
            .d
            .inventory
            .as_ref()
            .map(|i| {
                i.systems()
                    .into_iter()
                    .map(|s| SystemInfo {
                        name: s.name,
                        display_name: s.display_name,
                        interface: s.interface.to_string(),
                        capabilities: s.capabilities.iter().map(|c| c.to_string()).collect(),
                        data_sensitivity: s.data_sensitivity.iter().map(|d| d.to_string()).collect(),
                        owner_team: s.owner_team,
                        contact: s.contact,
                        auth_method: s.auth_method.to_string(),
                        failure_impact: s.failure_impact.to_string(),
                        realtime: s.realtime.to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let tools: Vec<ToolInfo> = self
            .d
            .registry
            .as_ref()
            .map(|r| r.specs().into_iter().map(ToolInfo::from).collect())
            .unwrap_or_default();

        let counts = match (&self.d.registry, &self.d.inventory) {
            | (Some(r), Some(i)) => crate::app::tool_counts(r, i),
            | _ => HashMap::new(),
        };

        (systems, tools, counts)
    }

    // --- 보존 기간 ---

    pub async fn retention(&self) -> Result<Vec<RetentionRow>> {
        let Some(reg) = &self.d.registry else {
            return Ok(Vec::new());
        };
        let oldest = self.d.audit.oldest().await?;
        let now = chrono::Utc::now();

        Ok(reg
            .specs()
            .into_iter()
            .map(|s| {
                let first = oldest.get(&s.name).copied();
                let over = match (s.log_retention_days > 0, first) {
                    | (true, Some(f)) => (now - f).num_days() > s.log_retention_days,
                    | _ => false,
                };
                RetentionRow {
                    tool: s.name,
                    days: s.log_retention_days,
                    oldest: first,
                    over,
                }
            })
            .collect())
    }

    // --- 대시보드 ---

    pub async fn dashboard(&self) -> Result<Dashboard> {
        let since = Some(chrono::Utc::now() - chrono::Duration::hours(DAY));
        let base = CallFilter {
            since,
            limit: 10_000,
            ..Default::default()
        };

        let mut d = Dashboard {
            calls: self.count_calls(&base).await?,
            denied: self
                .count_calls(&CallFilter {
                    decision: "denied".into(),
                    ..base.clone()
                })
                .await?,
            errors: self
                .count_calls(&CallFilter {
                    errors_only: true,
                    ..base.clone()
                })
                .await?,
            masked: self
                .count_calls(&CallFilter {
                    masked_only: true,
                    ..base.clone()
                })
                .await?,
            flagged: self
                .count_calls(&CallFilter {
                    injection_only: true,
                    ..base.clone()
                })
                .await?,
            pending: self.count_pending_approvals().await.unwrap_or(0),
            ..Default::default()
        };

        // **보상 실패한 흐름** — 사람이 손봐야 합니다.
        d.failed_flows = self.list_workflows("failed", 1000).await?.len();
        d.unhealthy = self.health().await.iter().filter(|h| !h.healthy).count();

        if let Some(b) = &self.d.budget {
            let mut costly = b.snapshot().await;
            costly.truncate(10);
            d.costly = costly;
        }

        if let Some(e) = &self.d.eval {
            d.eval_enabled = true;
            let f = eval::TurnFilter {
                since,
                limit: 10_000,
                ..Default::default()
            };
            d.eval_turns = e.query_turns(&f).await.map(|t| t.len()).unwrap_or(0);
            d.eval_unrated = e
                .query_turns(&eval::TurnFilter {
                    unrated_only: true,
                    ..f
                })
                .await
                .map(|t| t.len())
                .unwrap_or(0);

            let stats = e
                .stats(
                    eval::GroupBy::Source,
                    &eval::StatFilter {
                        source: eval::Source::HumanUser.as_str().into(),
                        scale: eval::Scale::Thumbs.as_str().into(),
                        since,
                    },
                )
                .await
                .unwrap_or_default();
            let (up, down): (i64, i64) = stats.iter().fold((0, 0), |(u, dn), s| (u + s.thumbs_up, dn + s.thumbs_down));
            d.eval_thumbs_rate = if up + down > 0 {
                format!("{:.0}%", (up as f64 / (up + down) as f64) * 100.0)
            } else {
                "-".into()
            };
        }

        Ok(d)
    }
}

/// 남은 시간 (사람이 읽는 문자열).
fn remaining(r: &approval::Request, now: DateTime<Utc>) -> String {
    match r.status {
        | Status::Expired => return "만료됨".into(),
        | Status::Consumed => return "사용됨".into(),
        | _ => {},
    }
    let Some(exp) = r.expires_at else {
        return "-".into();
    };
    if r.expired(now) {
        return "만료됨".into();
    }
    let d = exp - now;
    let secs = d.num_seconds().max(0);
    if secs >= 3600 {
        format!("{}시간 {}분", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}분 {}초", secs / 60, secs % 60)
    } else {
        format!("{secs}초")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn req(status: Status, expires_in: Option<i64>) -> approval::Request {
        let now = Utc.with_ymd_and_hms(2026, 7, 10, 9, 0, 0).unwrap();
        approval::Request {
            id: "req_1".into(),
            fingerprint: String::new(),
            actor: "emp-1".into(),
            tool: "t".into(),
            args: Map::new(),
            status,
            requested_at: now,
            decided_by: String::new(),
            decided_at: None,
            note: String::new(),
            ttl: Duration::from_secs(3600),
            expires_at: expires_in.map(|s| now + chrono::Duration::seconds(s)),
        }
    }

    fn now() -> DateTime<Utc> { Utc.with_ymd_and_hms(2026, 7, 10, 9, 0, 0).unwrap() }

    #[test]
    fn remaining_reports_terminal_states_plainly() {
        assert_eq!(remaining(&req(Status::Expired, None), now()), "만료됨");
        assert_eq!(remaining(&req(Status::Consumed, None), now()), "사용됨");
        assert_eq!(remaining(&req(Status::Pending, None), now()), "-");
    }

    #[test]
    fn remaining_counts_down() {
        assert_eq!(remaining(&req(Status::Approved, Some(3700)), now()), "1시간 1분");
        assert_eq!(remaining(&req(Status::Approved, Some(90)), now()), "1분 30초");
        assert_eq!(remaining(&req(Status::Approved, Some(30)), now()), "30초");
    }

    #[test]
    fn an_expired_approval_says_so_even_if_status_lags() {
        assert_eq!(remaining(&req(Status::Approved, Some(-1)), now()), "만료됨");
    }
}
