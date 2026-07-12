//! 운영 파사드.
//!
//! 콘솔(HTTP/HTML)과 CLI 가 `audit`·`approval`·`workflow`·`eval` 을 직접 만지지
//! 않도록, 조회·승인 결정·에이전트 카탈로그·설정 적용을 이 계층에 모읍니다.
//!
//! **설정 적용은 원자적 쓰기 + 실패 시 롤백입니다.** 임시 파일에 쓰고 rename
//! 하며, 리로드가 실패하면 이전 내용을 되돌리고 다시 리로드합니다 — 반쯤 적용된
//! 설정으로 게이트웨이가 도는 상태를 만들지 않기 위함입니다.

mod config_reload;
mod probe;
mod views;

use crate::{AdapterFactory,
            SystemsOptions,
            adapter::Adapter,
            approval,
            audit,
            auth::{self,
                   Identity,
                   TokenResolver},
            breaker,
            budget,
            eval,
            inventory::Inventory,
            policy,
            registry::Registry,
            toolcatalog,
            workflow};
use anyhow::{Result,
             anyhow,
             bail};
pub use config_reload::{ReloadBundleResult,
                        ReloadStepResult};
pub use probe::ProbeStatus;
use std::{path::PathBuf,
          sync::Arc};
pub use views::*;

/// 운영 파사드가 필요로 하는 것들.
pub struct Deps {
    pub audit: Arc<dyn audit::Reader>,
    pub approvals: Arc<dyn approval::Store>,
    pub workflows: Option<Arc<dyn workflow::Store>>,
    pub inventory: Option<Arc<Inventory>>,
    pub registry: Option<Arc<Registry>>,
    pub adapters: Vec<Arc<dyn Adapter>>,

    /// stdio 모드에서는 정적 스냅샷, HTTP 모드에서는 `tokens` 가 진짜입니다.
    pub principals: Vec<Identity>,
    pub tokens: Option<Arc<TokenResolver>>,
    pub principal_path: Option<PathBuf>,

    pub policy: Option<Arc<policy::Engine>>,
    pub policy_path: Option<PathBuf>,
    pub systems_path: Option<PathBuf>,

    pub catalog: Option<Arc<toolcatalog::Manager>>,
    pub reload_stamp_path: Option<PathBuf>,

    /// 인벤토리가 바뀌면 어댑터를 재배선합니다.
    pub adapter_factory: Option<Arc<dyn AdapterFactory>>,
    pub systems_options: SystemsOptions,

    pub roles: Vec<String>,
    pub budget: Option<Arc<dyn budget::Tracker>>,
    pub breakers: Option<Arc<breaker::Breaker>>,
    pub eval: Option<Arc<dyn eval::Store>>,
    /// 감사 기록용 (설정 적용 · 주체 변경).
    pub recorder: Option<Arc<dyn audit::Recorder>>,
}

/// 운영 파사드.
pub struct Service {
    pub(crate) d: Deps,
    /// 주체 파일 쓰기.
    pub(crate) principal_lock: tokio::sync::Mutex<()>,
    /// 정책·인벤토리·카탈로그·주체 YAML 적용 + 번들 리로드.
    pub(crate) config_lock: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("ops::Service").finish() }
}

/// 운영 오류 — 호출자가 종류를 구분해야 합니다.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("요청자는 자기 요청을 결정할 수 없습니다. 다른 사람이 검토해야 합니다(요청자: {0})")]
    SelfApproval(String),
    #[error("승인 요청이 이미 결정되었습니다")]
    NotPending,
    #[error("찾을 수 없습니다")]
    NotFound,
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<approval::Error> for Error {
    fn from(e: approval::Error) -> Self {
        match e {
            | approval::Error::SelfApproval(who) => Error::SelfApproval(who),
            | approval::Error::NotPending => Error::NotPending,
            | approval::Error::NotFound => Error::NotFound,
            | approval::Error::NoApprover => Error::Invalid("결정자가 필요합니다".into()),
            | other => Error::Other(anyhow!("{other}")),
        }
    }
}

impl Service {
    pub fn new(d: Deps) -> Result<Self> {
        Ok(Self {
            d,
            principal_lock: tokio::sync::Mutex::new(()),
            config_lock: tokio::sync::Mutex::new(()),
        })
    }

    pub fn deps(&self) -> &Deps { &self.d }

    pub fn eval_enabled(&self) -> bool { self.d.eval.is_some() }

    /// 현재 주체 목록 — HTTP 모드에서는 살아 있는 리졸버가 진실입니다.
    pub(crate) fn principals(&self) -> Vec<Identity> {
        match &self.d.tokens {
            | Some(t) => t.identities(),
            | None => self.d.principals.clone(),
        }
    }

    /// 설정 적용·주체 변경을 감사에 남깁니다.
    pub(crate) async fn audit_op(&self, actor: &str, tool: &str, decision: &str, reason: &str, error: &str) {
        let Some(rec) = &self.d.recorder else {
            return;
        };
        let entry = audit::Entry {
            timestamp: chrono::Utc::now(),
            actor: actor.to_string(),
            tool: tool.to_string(),
            system: "gateway".into(),
            access: "write".into(),
            decision: decision.to_string(),
            reason: reason.to_string(),
            approval_status: "n/a".into(),
            error: error.to_string(),
            ..Default::default()
        };
        if let Err(e) = rec.log(&entry).await {
            tracing::warn!("ops: audit {tool} failed: {e}");
        }
    }

    // --- 에이전트 ---

    /// 에이전트 카탈로그 (역할·도구·시스템·등록된 에이전트).
    pub fn agent_catalog(&self) -> AgentCatalog {
        let mut tools: Vec<String> = self.d.registry.as_ref().map(|r| r.names()).unwrap_or_default();
        tools.sort();

        let mut systems: Vec<String> = self
            .d
            .inventory
            .as_ref()
            .map(|i| i.systems().into_iter().map(|s| s.name).collect())
            .unwrap_or_default();
        systems.sort();

        let mut agents: Vec<AgentRow> = self.principals().into_iter().filter(|p| p.is_agent()).map(agent_row).collect();
        agents.sort_by(|a, b| a.user_id.cmp(&b.user_id));

        AgentCatalog {
            roles: self.d.roles.clone(),
            tools,
            systems,
            agents,
        }
    }

    pub fn find_agent(&self, user_id: &str) -> Option<AgentRow> { self.principals().into_iter().find(|p| p.is_agent() && p.user_id == user_id).map(agent_row) }

    pub fn principal_exists(&self, user_id: &str) -> bool { self.principals().iter().any(|p| p.user_id == user_id) }

    /// 에이전트의 스코프가 실재하는 도구·시스템·역할을 가리키는지 검증합니다.
    pub fn validate_agent_scope(&self, roles: &[String], tools: &[String], systems: &[String]) -> Result<()> {
        for r in roles {
            if !self.d.roles.contains(r) {
                bail!("알 수 없는 역할: {r:?}");
            }
        }
        let (Some(reg), Some(inv)) = (&self.d.registry, &self.d.inventory) else {
            return Ok(());
        };
        let candidate = Identity {
            user_id: "candidate".into(),
            kind: auth::KIND_AGENT.into(),
            allowed_tools: tools.to_vec(),
            allowed_systems: systems.to_vec(),
            ..Default::default()
        };
        policy::validate_allowlists(std::slice::from_ref(&candidate), reg, inv)
    }

    pub fn apply_capable(&self) -> bool { self.d.principal_path.is_some() }

    /// 에이전트를 추가·갱신합니다. **실패하면 파일을 되돌립니다.**
    pub async fn apply_agent(&self, actor: &str, id: &Identity, action: &str) -> Result<()> {
        if !id.is_agent() {
            bail!("ops: 에이전트(kind: agent)만 Apply 할 수 있습니다");
        }
        let Some(path) = &self.d.principal_path else {
            bail!("ops: principal 파일 경로가 없습니다");
        };
        self.validate_agent_scope(&id.roles, &id.allowed_tools, &id.allowed_systems)?;

        let _guard = self.principal_lock.lock().await;
        let backup = std::fs::read(path).ok();

        let result = (|| -> Result<()> {
            auth::upsert_principal_in_file(path, id)?;
            self.reload_tokens(path)
        })();

        match result {
            | Ok(()) => {
                self.audit_op(
                    actor,
                    "principal.apply",
                    "allowed",
                    &format!("principal apply {action} user_id={}", id.user_id),
                    "",
                )
                .await;
                Ok(())
            },
            | Err(e) => {
                // 되돌립니다 — 반쯤 적용된 주체 파일로 게이트웨이가 돌면 안 됩니다.
                if let Some(b) = backup {
                    let _ = auth::atomic_write_public(path, &b, 0o600);
                    let _ = self.reload_tokens(path);
                }
                self.audit_op(
                    actor,
                    "principal.apply",
                    "denied",
                    &format!("principal apply {action} user_id={}", id.user_id),
                    &e.to_string(),
                )
                .await;
                Err(e)
            },
        }
    }

    /// 에이전트를 제거합니다.
    pub async fn remove_agent(&self, actor: &str, user_id: &str) -> Result<()> {
        let Some(path) = &self.d.principal_path else {
            bail!("ops: principal 파일 경로가 없습니다");
        };
        if self.find_agent(user_id).is_none() {
            bail!("ops: 에이전트 {user_id:?} 을(를) 찾을 수 없습니다");
        }

        let _guard = self.principal_lock.lock().await;
        let backup = std::fs::read(path).ok();

        let result = (|| -> Result<()> {
            auth::remove_principal_from_file(path, user_id)?;
            self.reload_tokens(path)
        })();

        match result {
            | Ok(()) => {
                self.audit_op(actor, "principal.apply", "allowed", &format!("principal remove user_id={user_id}"), "")
                    .await;
                Ok(())
            },
            | Err(e) => {
                if let Some(b) = backup {
                    let _ = auth::atomic_write_public(path, &b, 0o600);
                    let _ = self.reload_tokens(path);
                }
                Err(e)
            },
        }
    }

    fn reload_tokens(&self, path: &std::path::Path) -> Result<()> {
        let Some(tokens) = &self.d.tokens else {
            return Ok(()); // 파일 전용 모드 — 운영자가 재기동합니다.
        };
        let reg = self.d.registry.clone();
        let inv = self.d.inventory.clone();
        let validate = move |ids: &[Identity]| -> Result<()> {
            if let (Some(reg), Some(inv)) = (&reg, &inv) {
                policy::validate_allowlists(ids, reg, inv)?;
            }
            Ok(())
        };
        tokens.reload(path, Some(&validate))?;
        Ok(())
    }
}

fn agent_row(p: Identity) -> AgentRow {
    AgentRow {
        has_token: !p.token_sha256.is_empty(),
        expiry: fmt_expiry(p.token_expires_at),
        user_id: p.user_id,
        roles: p.roles,
        allowed_tools: p.allowed_tools,
        allowed_systems: p.allowed_systems,
        token_sha256: p.token_sha256,
        token_expires_at: p.token_expires_at,
    }
}

fn fmt_expiry(t: Option<chrono::DateTime<chrono::Utc>>) -> String {
    let Some(t) = t else {
        return "무기한".into();
    };
    let s = t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string();
    if t < chrono::Utc::now() { format!("만료됨({s})") } else { s }
}
