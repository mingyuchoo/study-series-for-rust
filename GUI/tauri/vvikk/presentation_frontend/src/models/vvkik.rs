//! 와이어 타입은 contracts 크레이트가 단일 정의를 갖고, 프런트엔드는
//! 그대로 재노출해서 사용한다. 여기에는 화면 전용 도우미만 남긴다.

pub use contracts::{ApiError,
                    CreateItemRequest,
                    ItemKind,
                    ItemStatus,
                    KpiMeasurementDto as KpiMeasurement,
                    RecordKpiMeasurementRequest,
                    UpdateItemRequest,
                    VvkikItemDto as VvkikItem};

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
