//! 와이어 타입은 contracts 크레이트가 단일 정의를 갖고, 프런트엔드는
//! 그대로 재노출해서 사용한다. 여기에는 화면 전용 도우미만 남긴다.

pub use contracts::{ApiError,
                    CreateItemRequest,
                    ItemKind,
                    ItemStatus,
                    KpiAggregation,
                    KpiMeasurementDto as KpiMeasurement,
                    RecordKpiMeasurementRequest,
                    UpdateItemRequest,
                    VvkikItemDto as VvkikItem};

/// 상태의 화면용 레이블. 와이어 포맷(소문자)과 달리 대문자로 시작한다.
pub fn status_label(status: ItemStatus) -> &'static str {
    match status {
        | ItemStatus::Active => "Active",
        | ItemStatus::Paused => "Paused",
        | ItemStatus::Completed => "Completed",
    }
}

/// 집계 방식의 화면용 레이블.
pub fn aggregation_label(aggregation: KpiAggregation) -> &'static str {
    match aggregation {
        | KpiAggregation::Latest => "최신값",
        | KpiAggregation::Sum => "합계",
        | KpiAggregation::Average => "평균",
    }
}

/// 집계 방식의 화면용 설명 문구.
pub fn aggregation_description(aggregation: KpiAggregation) -> &'static str {
    match aggregation {
        | KpiAggregation::Latest => "가장 최근 기록값을 현재값으로 사용 (체지방률, 중량 등 수준 지표)",
        | KpiAggregation::Sum => "기록값을 모두 더해 현재값으로 사용 (커밋 수, 거리 등 누적 지표)",
        | KpiAggregation::Average => "기록값의 평균을 현재값으로 사용 (수면 시간 등 평균 지표)",
    }
}

/// 각 단계의 화면용 설명 문구.
pub fn kind_description(kind: ItemKind) -> &'static str {
    match kind {
        | ItemKind::Value => "무엇을 중요하게 여길 것인가?",
        | ItemKind::Vision => "어디로 갈 것인가?",
        | ItemKind::Kra => "반드시 집중해야 할 핵심 영역",
        | ItemKind::Igt => "실제 돈과 성과를 만드는 실행 업무",
        | ItemKind::Kpi => "측정과 피드백으로 시스템을 조정하는 지표",
    }
}
