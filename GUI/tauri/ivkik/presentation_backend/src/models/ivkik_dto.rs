//! 도메인 엔티티 → 공유 와이어 DTO 변환.
//!
//! DTO 타입 자체는 `contracts` 크레이트가 단일 정의를 갖는다. 엔티티와
//! DTO 모두 외부 크레이트 타입이라(orphan rule) `From` 대신 자유 함수로
//! 변환한다.

pub use contracts::{CreateItemRequest,
                    ItemRevisionDto,
                    IvkikItemDto,
                    KpiMeasurementDto,
                    RecordKpiMeasurementRequest,
                    UpdateItemRequest};
use domain::{ItemRevision,
             IvkikItem,
             KpiMeasurement};

pub fn kind_to_domain(kind: contracts::ItemKind) -> domain::ItemKind {
    match kind {
        | contracts::ItemKind::Identity => domain::ItemKind::Identity,
        | contracts::ItemKind::Vision => domain::ItemKind::Vision,
        | contracts::ItemKind::Kra => domain::ItemKind::Kra,
        | contracts::ItemKind::Igt => domain::ItemKind::Igt,
        | contracts::ItemKind::Kpi => domain::ItemKind::Kpi,
    }
}

pub fn status_to_domain(status: contracts::ItemStatus) -> domain::ItemStatus {
    match status {
        | contracts::ItemStatus::Active => domain::ItemStatus::Active,
        | contracts::ItemStatus::Paused => domain::ItemStatus::Paused,
        | contracts::ItemStatus::Completed => domain::ItemStatus::Completed,
    }
}

pub fn aggregation_to_domain(aggregation: contracts::KpiAggregation) -> domain::KpiAggregation {
    match aggregation {
        | contracts::KpiAggregation::Latest => domain::KpiAggregation::Latest,
        | contracts::KpiAggregation::Sum => domain::KpiAggregation::Sum,
        | contracts::KpiAggregation::Average => domain::KpiAggregation::Average,
    }
}

fn kind_to_dto(kind: domain::ItemKind) -> contracts::ItemKind {
    match kind {
        | domain::ItemKind::Identity => contracts::ItemKind::Identity,
        | domain::ItemKind::Vision => contracts::ItemKind::Vision,
        | domain::ItemKind::Kra => contracts::ItemKind::Kra,
        | domain::ItemKind::Igt => contracts::ItemKind::Igt,
        | domain::ItemKind::Kpi => contracts::ItemKind::Kpi,
    }
}

fn status_to_dto(status: domain::ItemStatus) -> contracts::ItemStatus {
    match status {
        | domain::ItemStatus::Active => contracts::ItemStatus::Active,
        | domain::ItemStatus::Paused => contracts::ItemStatus::Paused,
        | domain::ItemStatus::Completed => contracts::ItemStatus::Completed,
    }
}

fn aggregation_to_dto(aggregation: domain::KpiAggregation) -> contracts::KpiAggregation {
    match aggregation {
        | domain::KpiAggregation::Latest => contracts::KpiAggregation::Latest,
        | domain::KpiAggregation::Sum => contracts::KpiAggregation::Sum,
        | domain::KpiAggregation::Average => contracts::KpiAggregation::Average,
    }
}

pub fn item_to_dto(item: IvkikItem) -> IvkikItemDto {
    IvkikItemDto {
        id: item.id.to_string(),
        kind: kind_to_dto(item.kind),
        parent_id: item.parent_id.map(|id| id.to_string()),
        title: item.title,
        description: item.description,
        target_value: item.target_value,
        current_value: item.current_value,
        unit: item.unit,
        position: item.position,
        status: status_to_dto(item.status),
        aggregation: aggregation_to_dto(item.aggregation),
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
