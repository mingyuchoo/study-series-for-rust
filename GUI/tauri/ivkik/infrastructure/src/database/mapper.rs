use chrono::{DateTime,
             Utc};
use domain::{DomainError,
             ItemKind,
             ItemRevision,
             ItemStatus,
             IvkikItem,
             KpiAggregation,
             KpiMeasurement};
use sqlx::{Row,
           sqlite::SqliteRow};
use std::str::FromStr;
use uuid::Uuid;

fn parse_datetime(value: &str) -> Result<DateTime<Utc>, DomainError> {
    DateTime::parse_from_rfc3339(value)
        .map_err(|e| DomainError::DatabaseError(e.to_string()))
        .map(|value| value.with_timezone(&Utc))
}

pub fn row_to_item(row: &SqliteRow) -> Result<IvkikItem, DomainError> {
    Ok(IvkikItem {
        id: Uuid::parse_str(row.get("id")).map_err(|e| DomainError::DatabaseError(e.to_string()))?,
        kind: ItemKind::from_str(row.get("kind")).map_err(DomainError::DatabaseError)?,
        parent_id: row
            .get::<Option<String>, _>("parent_id")
            .map(|id| Uuid::parse_str(&id).map_err(|e| DomainError::DatabaseError(e.to_string())))
            .transpose()?,
        title: row.get("title"),
        description: row.get("description"),
        target_value: row.get("target_value"),
        current_value: row.get("current_value"),
        unit: row.get("unit"),
        position: row.get("position"),
        status: ItemStatus::from_str(row.get("status")).map_err(DomainError::DatabaseError)?,
        aggregation: KpiAggregation::from_str(row.get("aggregation")).map_err(DomainError::DatabaseError)?,
        created_at: parse_datetime(row.get("created_at"))?,
        updated_at: parse_datetime(row.get("updated_at"))?,
    })
}

pub fn row_to_measurement(row: &SqliteRow) -> Result<KpiMeasurement, DomainError> {
    Ok(KpiMeasurement {
        id: Uuid::parse_str(row.get("id")).map_err(|e| DomainError::DatabaseError(e.to_string()))?,
        kpi_id: Uuid::parse_str(row.get("kpi_id")).map_err(|e| DomainError::DatabaseError(e.to_string()))?,
        value: row.get("value"),
        measured_at: parse_datetime(row.get("measured_at"))?,
        note: row.get("note"),
    })
}

pub fn row_to_revision(row: &SqliteRow) -> Result<ItemRevision, DomainError> {
    Ok(ItemRevision {
        id: Uuid::parse_str(row.get("id")).map_err(|e| DomainError::DatabaseError(e.to_string()))?,
        item_id: Uuid::parse_str(row.get("item_id")).map_err(|e| DomainError::DatabaseError(e.to_string()))?,
        field: row.get("field"),
        old_value: row.get("old_value"),
        new_value: row.get("new_value"),
        changed_at: parse_datetime(row.get("changed_at"))?,
    })
}
