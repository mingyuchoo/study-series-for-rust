//! 도메인 엔티티 → 공유 와이어 DTO 변환.
//!
//! DTO 타입 자체는 `contracts` 크레이트가 단일 정의를 갖는다. 단계·상태·
//! 집계 방식 enum도 contracts의 것을 도메인이 그대로 재사용하므로
//! 변환할 것은 id·시각의 문자열 표현뿐이다. 엔티티와 DTO 모두 외부
//! 크레이트 타입이라(orphan rule) `From` 대신 자유 함수로 변환한다.

pub use contracts::{CreateItemRequest,
                    IkikItemDto,
                    ItemRevisionDto,
                    KpiMeasurementDto,
                    RecordKpiMeasurementRequest,
                    UpdateItemRequest};
use domain::{IkikItem,
             ItemRevision,
             KpiMeasurement};

pub fn item_to_dto(item: IkikItem) -> IkikItemDto {
    IkikItemDto {
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
        aggregation: item.aggregation,
        due_date: item.due_date.map(|date| date.to_string()),
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

pub fn revision_to_dto(revision: ItemRevision) -> ItemRevisionDto {
    ItemRevisionDto {
        id: revision.id.to_string(),
        item_id: revision.item_id.to_string(),
        field: revision.field,
        old_value: revision.old_value,
        new_value: revision.new_value,
        changed_at: revision.changed_at.to_rfc3339(),
    }
}
