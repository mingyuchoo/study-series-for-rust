use domain::{ItemKind,
             ItemStatus,
             KpiMeasurement,
             VvkikItem};
use serde::{Deserialize,
            Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct VvkikItemDto {
    pub id: String,
    pub kind: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub target_value: Option<f64>,
    pub current_value: Option<f64>,
    pub unit: Option<String>,
    pub position: i64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateItemRequest {
    pub kind: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub target_value: Option<f64>,
    pub current_value: Option<f64>,
    pub unit: Option<String>,
    pub position: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateItemRequest {
    pub id: String,
    pub kind: Option<String>,
    pub parent_id: Option<Option<String>>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub target_value: Option<Option<f64>>,
    pub current_value: Option<Option<f64>>,
    pub unit: Option<String>,
    pub position: Option<i64>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KpiMeasurementDto {
    pub id: String,
    pub kpi_id: String,
    pub value: f64,
    pub measured_at: String,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecordKpiMeasurementRequest {
    pub kpi_id: String,
    pub value: f64,
    pub note: Option<String>,
}

impl From<VvkikItem> for VvkikItemDto {
    fn from(item: VvkikItem) -> Self {
        Self {
            id: item.id.to_string(),
            kind: item.kind.as_str().to_string(),
            parent_id: item.parent_id.map(|id| id.to_string()),
            title: item.title,
            description: item.description,
            target_value: item.target_value,
            current_value: item.current_value,
            unit: item.unit,
            position: item.position,
            status: item.status.as_str().to_string(),
            created_at: item.created_at.to_rfc3339(),
            updated_at: item.updated_at.to_rfc3339(),
        }
    }
}

impl From<KpiMeasurement> for KpiMeasurementDto {
    fn from(measurement: KpiMeasurement) -> Self {
        Self {
            id: measurement.id.to_string(),
            kpi_id: measurement.kpi_id.to_string(),
            value: measurement.value,
            measured_at: measurement.measured_at.to_rfc3339(),
            note: measurement.note,
        }
    }
}

pub fn parse_kind(kind: &str) -> Result<ItemKind, String> { kind.parse() }

pub fn parse_status(status: &str) -> Result<ItemStatus, String> { status.parse() }
