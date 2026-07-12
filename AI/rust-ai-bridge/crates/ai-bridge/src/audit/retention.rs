//! 보존 기간 집행 — 내보낸 뒤 지웁니다.
//!
//! 도구마다 보존 기간을 선언합니다(`Spec.log_retention_days`). **선언해놓고
//! 아무도 읽지 않으면 그 선언은 주석입니다.**
//!
//! 레지스트리에서 사라진 옛 도구의 기록은 **기본적으로 지우지
//! 않습니다**(`default: 0`). 그러지 않으면 도구 이름을 바꾸는 것만으로 감사
//! 기록이 사라집니다.

use super::{Entry,
            Exporter};
use anyhow::{Result,
             anyhow};
use chrono::{DateTime,
             Duration,
             Utc};
use std::collections::HashMap;

/// 한 배치에 내보내고 지우는 건수.
pub(crate) const PURGE_BATCH: i64 = 500;

/// 도구별 보존 기간.
#[derive(Debug, Clone, Default)]
pub struct Policy {
    /// 도구 → 보존 일수. `<= 0` 이면 지우지 않습니다.
    pub by_tool: HashMap<String, i64>,
    /// 목록에 없는 도구의 보존 일수. **0 이면 영구 보존**(기본).
    pub default: i64,
}

/// 삭제 결과.
#[derive(Debug, Clone, Default)]
pub struct Purged {
    pub deleted: i64,
    pub by_tool: HashMap<String, i64>,
    /// 보존 일수가 0 이하라 건드리지 않은 도구들.
    pub skipped: Vec<String>,
}

/// 저장소가 삭제를 위해 제공해야 하는 최소 동작.
#[async_trait::async_trait]
pub(crate) trait PurgeBackend: Send + Sync {
    /// 조건에 맞는 기록을 오래된 것부터 `limit` 건 가져옵니다.
    async fn select_for_archive(&self, tool: Option<&str>, exclude_tools: &[String], before: DateTime<Utc>, limit: i64) -> Result<Vec<Entry>>;

    /// **정확히 이 id 들만** 지웁니다.
    async fn delete_by_ids(&self, ids: &[i64]) -> Result<i64>;
}

/// 보존 기간을 집행합니다.
///
/// 순서가 계약입니다: **내보내기 → 삭제**. 내보내기가 실패하면 그 배치는 남고
/// Purge 가 중단됩니다. 이미 아카이브되어 지워진 앞선 배치는 파일에 있으므로
/// 유실이 없습니다.
///
/// 내보내기와 삭제 사이에 프로세스가 죽으면 같은 기록이 다음 실행에서 다시
/// 내보내집니다. **최소 한 번(at-least-once) 전달**이며, 중복은 있어도 유실은
/// 없습니다. 수신 측은 기록 `id` 로 중복을 걸러야 합니다.
pub(crate) async fn purge(backend: &dyn PurgeBackend, p: &Policy, now: DateTime<Utc>, exp: &dyn Exporter) -> Result<Purged> {
    let mut out = Purged::default();

    // 도구 이름순 — 결과가 실행마다 달라지지 않도록.
    let mut tools: Vec<(&String, &i64)> = p.by_tool.iter().collect();
    tools.sort_by(|a, b| a.0.cmp(b.0));

    for (tool, days) in &tools {
        if **days <= 0 {
            // 보존 일수 0 = 영구 보존.
            out.skipped.push((*tool).clone());
            continue;
        }
        let cutoff = now - Duration::days(**days);
        let n = archive_and_delete(backend, exp, Some(tool), &[], cutoff, &mut out).await?;
        if n > 0 {
            out.by_tool.insert((*tool).clone(), n);
        }
    }

    if p.default > 0 {
        // 명시적으로 나열된 도구는 (보존 0인 것까지 포함해) 기본 정책에서 제외합니다.
        let known: Vec<String> = tools.iter().map(|(t, _)| (*t).clone()).collect();
        let cutoff = now - Duration::days(p.default);
        archive_and_delete(backend, exp, None, &known, cutoff, &mut out).await?;
    }

    Ok(out)
}

async fn archive_and_delete(
    backend: &dyn PurgeBackend,
    exp: &dyn Exporter,
    tool: Option<&str>,
    exclude: &[String],
    before: DateTime<Utc>,
    out: &mut Purged,
) -> Result<i64> {
    let mut total = 0i64;
    loop {
        let batch = backend.select_for_archive(tool, exclude, before, PURGE_BATCH).await?;
        if batch.is_empty() {
            return Ok(total);
        }

        // 먼저 내보냅니다. 실패하면 **아무것도 지우지 않고** 중단합니다.
        exp.export(&batch).await.map_err(|e| anyhow!("audit: archive before purge: {e}"))?;

        // 방금 내보낸 id 만 지웁니다 — 그 사이 들어온 기록이 휩쓸리지 않도록.
        let ids: Vec<i64> = batch.iter().map(|e| e.id).collect();
        let n = backend.delete_by_ids(&ids).await?;
        total += n;
        out.deleted += n;
    }
}

/// `days` 일 전 시각.
pub fn cutoff(now: DateTime<Utc>, days: i64) -> DateTime<Utc> { now - Duration::days(days) }
