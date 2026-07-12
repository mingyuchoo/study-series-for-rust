//! 승인 관문.

use crate::{approval,
            auth::Identity,
            registry::Spec};
use anyhow::Result;
use serde_json::{Map,
                 Value};
use std::time::Duration;

/// 승인 상태.
#[derive(Debug, Clone, Default)]
pub struct Approval {
    /// **지금 실행해도 되는가.**
    pub approved: bool,
    /// `pending` | `approved` | `rejected`.
    pub status: String,
    pub request_id: String,
    pub ttl: Duration,
}

/// 이 호출이 승인되었는지 확인하고, 승인되었다면 **소비합니다.**
#[async_trait::async_trait]
pub trait Approver: Send + Sync {
    async fn approve(&self, id: &Identity, spec: &Spec, args: &Map<String, Value>) -> Result<Approval>;
}

/// 아무것도 승인하지 않습니다. **기본값이며 fail-closed 입니다.**
///
/// 승인 저장소를 꽂지 않은 게이트웨이에서 L3+ 도구가 실행되는 일은 없습니다.
#[derive(Debug, Clone, Copy, Default)]
pub struct DenyApprover;

#[async_trait::async_trait]
impl Approver for DenyApprover {
    async fn approve(&self, _id: &Identity, _spec: &Spec, _args: &Map<String, Value>) -> Result<Approval> {
        Ok(Approval {
            approved: false,
            status: approval::Status::Pending.as_str().to_string(),
            ..Default::default()
        })
    }
}

/// 승인 저장소에 위임합니다.
pub struct StoreApprover {
    store: std::sync::Arc<dyn approval::Store>,
}

impl std::fmt::Debug for StoreApprover {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("StoreApprover").finish() }
}

impl StoreApprover {
    pub fn new(store: std::sync::Arc<dyn approval::Store>) -> Self {
        Self {
            store,
        }
    }
}

#[async_trait::async_trait]
impl Approver for StoreApprover {
    async fn approve(&self, id: &Identity, spec: &Spec, args: &Map<String, Value>) -> Result<Approval> {
        // `ensure` 가 승인을 **원자적으로 소비**합니다 — 두 프로세스가 같은 승인으로
        // 도구를 두 번 실행할 수 없습니다.
        let req = self
            .store
            .ensure(&id.user_id, &spec.name, args, spec.approval_ttl)
            .await
            .map_err(|e| anyhow::anyhow!("승인 요청 확인 실패: {e}"))?;

        Ok(Approval {
            approved: req.status == approval::Status::Approved,
            status: req.status.as_str().to_string(),
            request_id: req.id,
            ttl: req.ttl,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_default_approver_denies_everything() {
        // 승인 저장소 없이 뜬 게이트웨이에서 L3+ 가 실행되면 안 됩니다.
        let a = DenyApprover.approve(&Identity::default(), &Spec::default(), &Map::new()).await.unwrap();
        assert!(!a.approved);
        assert_eq!(a.status, "pending");
    }
}
