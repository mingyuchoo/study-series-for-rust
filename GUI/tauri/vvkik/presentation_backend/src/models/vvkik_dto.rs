//! 도메인 엔티티 → 공유 와이어 DTO 변환.
//!
//! DTO 타입 자체는 `contracts` 크레이트가 단일 정의를 갖는다. 엔티티와
//! DTO 모두 외부 크레이트 타입이라(orphan rule) `From` 대신 자유 함수로
//! 변환한다.

pub use contracts::{CreateItemRequest,
                    KpiMeasurementDto,
                    RecordKpiMeasurementRequest,
                    UpdateItemRequest,
                    VvkikItemDto};
use domain::{KpiMeasurement,
             VvkikItem};

pub fn item_to_dto(item: VvkikItem) -> VvkikItemDto {
    VvkikItemDto {
        id: item.id.to_string(),
        kind: item.kind,
        parent_id: item.parent_id.map(|id| id.to_string()),
        title: item.title,
        description: item.description,
        target_value: item.target_value,
        current_value: item.current_value,
        unit: item.unit,
        position: item.position,
        status: item.status,
        created_at: item.created_at.to_rfc3339(),
        updated_at: item.updated_at.to_rfc3339(),
    }
}

pub fn measurement_to_dto(measurement: KpiMeasurement) -> KpiMeasurementDto {
    KpiMeasurementDto {
        id: measurement.id.to_string(),
        kpi_id: measurement.kpi_id.to_string(),
        value: measurement.value,
        measured_at: measurement.measured_at.to_rfc3339(),
        note: measurement.note,
    }
}
