//! 와이어 타입은 contracts 크레이트가 단일 정의를 갖고, 프런트엔드는
//! 그대로 재노출해서 사용한다. 여기에는 화면 전용 도우미만 남긴다.

pub use contracts::{ApiError,
                    CreateItemRequest,
                    ItemKind,
                    ItemRevisionDto as ItemRevision,
                    ItemStatus,
                    IvkikItemDto as IvkikItem,
                    KpiAggregation,
                    KpiMeasurementDto as KpiMeasurement,
                    RecordKpiMeasurementRequest,
                    UpdateItemRequest};

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

/// 변경 이력의 와이어 필드 이름 → 화면 레이블.
pub fn revision_field_label(field: &str) -> &'static str {
    match field {
        | "kind" => "단계",
        | "parent" => "상위 항목",
        | "title" => "제목",
        | "description" => "설명",
        | "target_value" => "목표값",
        | "unit" => "단위",
        | "status" => "상태",
        | "aggregation" => "집계 방식",
        | _ => "기타",
    }
}

/// 변경 이력 값의 화면 표기. enum 필드는 와이어 값(latest 등)을
/// 화면 레이블로 바꾸고, 빈 값은 "없음"으로 보여 준다.
pub fn revision_value_label(field: &str, value: Option<&str>) -> String {
    let Some(value) = value else {
        return "없음".to_string();
    };

    match field {
        | "kind" => value
            .parse::<ItemKind>()
            .map(|kind| kind.label().to_string())
            .unwrap_or_else(|_| value.to_string()),
        | "status" => value
            .parse::<ItemStatus>()
            .map(|status| status_label(status).to_string())
            .unwrap_or_else(|_| value.to_string()),
        | "aggregation" => value
            .parse::<KpiAggregation>()
            .map(|aggregation| aggregation_label(aggregation).to_string())
            .unwrap_or_else(|_| value.to_string()),
        | _ => value.to_string(),
    }
}

/// 각 단계의 화면용 설명 문구.
pub fn kind_description(kind: ItemKind) -> &'static str {
    match kind {
        | ItemKind::Identity => "나는 어떤 사람이 될 것인가?",
        | ItemKind::Vision => "어디로 갈 것인가?",
        | ItemKind::Kra => "반드시 집중해야 할 핵심 영역",
        | ItemKind::Igt => "실제 돈과 성과를 만드는 실행 업무",
        | ItemKind::Kpi => "측정과 피드백으로 시스템을 조정하는 지표",
    }
}
