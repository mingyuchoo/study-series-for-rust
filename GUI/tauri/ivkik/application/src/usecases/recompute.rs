use domain::{DomainError,
             ItemKind,
             ItemPatch,
             IvkikItem,
             IvkikRepository};
use uuid::Uuid;

/// KPI의 측정 기록 전체를 집계 방식대로 취합해 현재값을 다시 쓴다.
///
/// 기록이 하나도 없으면 `reset_when_empty`가 참일 때만 현재값을
/// 비운다 — 기록 없이 수동으로 입력해 둔 현재값을 망가뜨리지 않기
/// 위해서다. 현재값을 다시 썼으면 갱신된 항목을 돌려준다.
pub(crate) async fn recompute_kpi_current_value(
    repository: &dyn IvkikRepository,
    kpi_id: Uuid,
    reset_when_empty: bool,
) -> Result<Option<IvkikItem>, DomainError> {
    let mut kpi = repository.get_item_by_id(kpi_id).await?.ok_or(DomainError::ItemNotFound)?;
    if kpi.kind != ItemKind::Kpi {
        return Ok(None);
    }

    // 리포지토리가 최신순(측정 시각 내림차순)으로 돌려준다.
    let measurements = repository.list_kpi_measurements(kpi_id).await?;
    let values: Vec<f64> = measurements.iter().map(|measurement| measurement.value).collect();
    let aggregated = kpi.aggregation.aggregate(&values);

    if aggregated.is_none() && !reset_when_empty {
        return Ok(None);
    }

    kpi.update(ItemPatch {
        current_value: Some(aggregated),
        ..ItemPatch::default()
    });
    repository.update_item(kpi).await.map(Some)
}
