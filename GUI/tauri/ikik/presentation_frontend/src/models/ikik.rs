//! 와이어 타입은 contracts 크레이트가 단일 정의를 갖고, 프런트엔드는
//! 그대로 재노출해서 사용한다. 여기에는 화면 전용 도우미만 남긴다.

use crate::i18n::Lang;
pub use contracts::{ApiError,
                    CreateItemRequest,
                    IkikItemDto as IkikItem,
                    ItemKind,
                    ItemRevisionDto as ItemRevision,
                    ItemStatus,
                    KpiAggregation,
                    KpiMeasurementDto as KpiMeasurement,
                    RecordKpiMeasurementRequest,
                    UpdateItemRequest};

/// 단계의 화면용 레이블. 한국어 화면은 우리말 명칭을, 영어 화면은
/// 와이어 정의(contracts)의 영문 풀네임을 쓴다.
pub fn kind_label(kind: ItemKind, lang: Lang) -> &'static str {
    match (lang, kind) {
        | (Lang::Ko, ItemKind::Identity) => "정체성",
        | (Lang::Ko, ItemKind::Kra) => "핵심 결과 영역",
        | (Lang::Ko, ItemKind::Igt) => "소득 창출 업무",
        | (Lang::Ko, ItemKind::Kpi) => "핵심 성과 지표",
        | (Lang::En, _) => kind.label(),
    }
}

/// 상태의 화면용 레이블. 한국어 화면은 우리말 명칭을 쓴다.
pub fn status_label(status: ItemStatus, lang: Lang) -> &'static str {
    match (lang, status) {
        | (Lang::Ko, ItemStatus::Active) => "진행 중",
        | (Lang::Ko, ItemStatus::Paused) => "일시 중지",
        | (Lang::Ko, ItemStatus::Completed) => "완료",
        | (Lang::En, ItemStatus::Active) => "Active",
        | (Lang::En, ItemStatus::Paused) => "Paused",
        | (Lang::En, ItemStatus::Completed) => "Completed",
    }
}

/// 집계 방식의 화면용 레이블.
pub fn aggregation_label(aggregation: KpiAggregation, lang: Lang) -> &'static str {
    match (lang, aggregation) {
        | (Lang::Ko, KpiAggregation::Latest) => "최신값",
        | (Lang::Ko, KpiAggregation::Sum) => "합계",
        | (Lang::Ko, KpiAggregation::Average) => "평균",
        | (Lang::En, KpiAggregation::Latest) => "Latest",
        | (Lang::En, KpiAggregation::Sum) => "Sum",
        | (Lang::En, KpiAggregation::Average) => "Average",
    }
}

/// 집계 방식의 화면용 설명 문구.
pub fn aggregation_description(aggregation: KpiAggregation, lang: Lang) -> &'static str {
    match (lang, aggregation) {
        | (Lang::Ko, KpiAggregation::Latest) => "가장 최근 기록값을 현재값으로 사용 (체지방률, 중량 등 수준 지표)",
        | (Lang::Ko, KpiAggregation::Sum) => "기록값을 모두 더해 현재값으로 사용 (커밋 수, 거리 등 누적 지표)",
        | (Lang::Ko, KpiAggregation::Average) => "기록값의 평균을 현재값으로 사용 (수면 시간 등 평균 지표)",
        | (Lang::En, KpiAggregation::Latest) => "Use the most recent value as the current value (level metrics such as body fat or weight)",
        | (Lang::En, KpiAggregation::Sum) => "Add every value into the current value (cumulative metrics such as commits or distance)",
        | (Lang::En, KpiAggregation::Average) => "Use the average of values as the current value (average metrics such as sleep hours)",
    }
}

/// 변경 이력의 와이어 필드 이름 → 화면 레이블.
pub fn revision_field_label(field: &str, lang: Lang) -> &'static str {
    match field {
        | "kind" => lang.stage_label(),
        | "parent" => lang.parent_label(),
        | "title" => lang.title_label(),
        | "description" => lang.description_label(),
        | "target_value" => lang.target_label(),
        | "unit" => lang.unit_label(),
        | "status" => lang.status_field(),
        | "aggregation" => lang.aggregation_field(),
        | "due_date" => lang.due_date_label(),
        | _ => match lang {
            | Lang::Ko => "기타",
            | Lang::En => "Other",
        },
    }
}

/// 변경 이력 값의 화면 표기. enum 필드는 와이어 값(latest 등)을
/// 화면 레이블로 바꾸고, 빈 값은 "없음"으로 보여 준다.
pub fn revision_value_label(field: &str, value: Option<&str>, lang: Lang) -> String {
    let Some(value) = value else {
        return lang.none_label().to_string();
    };

    match field {
        | "kind" => value
            .parse::<ItemKind>()
            .map(|kind| kind_label(kind, lang).to_string())
            .unwrap_or_else(|_| value.to_string()),
        | "status" => value
            .parse::<ItemStatus>()
            .map(|status| status_label(status, lang).to_string())
            .unwrap_or_else(|_| value.to_string()),
        | "aggregation" => value
            .parse::<KpiAggregation>()
            .map(|aggregation| aggregation_label(aggregation, lang).to_string())
            .unwrap_or_else(|_| value.to_string()),
        | _ => value.to_string(),
    }
}

/// 각 단계의 화면용 설명 문구.
pub fn kind_description(kind: ItemKind, lang: Lang) -> &'static str {
    match (lang, kind) {
        | (Lang::Ko, ItemKind::Identity) => "나는 어떤 사람이 될 것인가?",
        | (Lang::Ko, ItemKind::Kra) => "반드시 집중해야 할 핵심 영역",
        | (Lang::Ko, ItemKind::Igt) => "실제 돈과 성과를 만드는 실행 업무",
        | (Lang::Ko, ItemKind::Kpi) => "측정과 피드백으로 시스템을 조정하는 지표",
        | (Lang::En, ItemKind::Identity) => "Who am I going to become?",
        | (Lang::En, ItemKind::Kra) => "Key areas that demand focus",
        | (Lang::En, ItemKind::Igt) => "Execution work that produces real money and results",
        | (Lang::En, ItemKind::Kpi) => "Metrics that tune the system through measurement and feedback",
    }
}
