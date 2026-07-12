//! 어댑터·레지스트리 조립과 기동 시 교차 검증.
//!
//! **인벤토리와 코드가 어긋나면 게이트웨이가 기동하지 않습니다.** 여기 있는
//! 검사들이 그것을 강제합니다 — 인벤토리에 없는 시스템의 도구는 등록되지 않고,
//! `capabilities` 에 쓰기 기능이 없으면 쓰기 도구를 붙일 수 없습니다.

use crate::{adapter::{Adapter,
                      Resource},
            audit,
            inventory::Inventory,
            registry::{Access,
                       Registry}};
use anyhow::{Result,
             anyhow,
             bail};
use std::{collections::HashMap,
          sync::Arc,
          time::{Duration,
                 Instant}};

/// 어댑터들의 도구를 모두 등록합니다. 첫 실패에서 멈춥니다.
pub fn build_registry(adapters: &[Arc<dyn Adapter>]) -> Result<Registry> {
    let reg = Registry::new();
    for a in adapters {
        for t in a.tools() {
            let name = t.spec.name.clone();
            reg.register(t).map_err(|e| anyhow!("adapter {}: {name}: {e}", a.name()))?;
        }
    }
    Ok(reg)
}

/// 등록된 도구가 인벤토리와 일치하는지 검사합니다.
///
/// 이 두 검사가 인벤토리를 **실제 통제 수단**으로 만듭니다.
pub fn validate_inventory(reg: &Registry, inv: &Inventory) -> Result<()> {
    for spec in reg.specs() {
        // 인벤토리에 없는 시스템의 도구는 등록될 수 없습니다.
        let Some(sys) = inv.lookup(&spec.system) else {
            bail!("인벤토리 불일치: 도구 {:?}: 시스템 {:?} 이(가) 인벤토리에 없습니다", spec.name, spec.system);
        };
        // 쓰기 기능이 선언되지 않은 시스템에 쓰기 도구를 붙일 수 없습니다.
        if spec.access == Access::Write && !sys.allows_write() {
            let caps: Vec<String> = sys.capabilities.iter().map(|c| c.to_string()).collect();
            bail!(
                "인벤토리 불일치: 도구 {:?}: 시스템 {:?} 의 capabilities에 쓰기 기능이 없습니다(현재 [{}])",
                spec.name,
                spec.system,
                caps.join(" ")
            );
        }
    }
    Ok(())
}

/// 도구가 하나도 없는 시스템 (기동 시 경고, 치명적이지 않음).
pub fn unused_systems(reg: &Registry, inv: &Inventory) -> Vec<String> {
    let used: std::collections::HashSet<String> = reg.specs().into_iter().map(|s| s.system).collect();
    inv.systems().into_iter().map(|s| s.name).filter(|n| !used.contains(n)).collect()
}

/// 도구별 보존 기간을 감사 정책으로 옮깁니다.
///
/// **레지스트리에서 사라진 옛 도구의 기록은 기본적으로 지우지
/// 않습니다**(`default: 0`) — 그러지 않으면 도구 이름을 바꾸는 것만으로 감사
/// 기록이 사라집니다.
pub fn retention_policy(reg: &Registry) -> audit::Policy {
    audit::Policy {
        by_tool: reg.specs().into_iter().map(|s| (s.name, s.log_retention_days)).collect(),
        default: 0,
    }
}

/// 레거시 시스템 상태.
#[derive(Debug, Clone)]
pub struct Health {
    pub system: String,
    pub healthy: bool,
    pub latency: Duration,
    pub error: String,
}

/// 모든 어댑터를 **동시에** 확인합니다.
pub async fn check_health(adapters: &[Arc<dyn Adapter>], timeout: Duration) -> Vec<Health> {
    let futures = adapters.iter().map(|a| {
        let a = a.clone();
        async move {
            let start = Instant::now();
            let res = tokio::time::timeout(timeout, a.health_check()).await;
            let (healthy, error) = match res {
                | Ok(Ok(())) => (true, String::new()),
                | Ok(Err(e)) => (false, e.to_string()),
                | Err(_) => (false, "health check timed out".to_string()),
            };
            Health {
                system: a.name(),
                healthy,
                latency: start.elapsed(),
                error,
            }
        }
    });
    let mut out: Vec<Health> = futures::future::join_all(futures).await;
    out.sort_by(|a, b| a.system.cmp(&b.system));
    out
}

/// 어댑터들이 노출하는 MCP 자원을 모읍니다.
pub fn resources(adapters: &[Arc<dyn Adapter>]) -> Vec<Resource> {
    let mut out: Vec<Resource> = adapters.iter().flat_map(|a| a.resources()).collect();
    out.sort_by(|a, b| a.uri.cmp(&b.uri));
    out
}

/// 시스템별 도구 수.
pub fn tool_counts(reg: &Registry, inv: &Inventory) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = inv.systems().into_iter().map(|s| (s.name, 0usize)).collect();
    for spec in reg.specs() {
        *counts.entry(spec.system).or_insert(0) += 1;
    }
    counts
}
