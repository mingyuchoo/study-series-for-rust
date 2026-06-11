use super::normalize_optional_field;
use chrono::{DateTime,
             Utc};
use serde::{Deserialize,
            Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KpiMeasurement {
    pub id: Uuid,
    pub kpi_id: Uuid,
    pub value: f64,
    pub measured_at: DateTime<Utc>,
    pub note: Option<String>,
}

impl KpiMeasurement {
    pub fn new(kpi_id: Uuid, value: f64, note: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            kpi_id,
            value,
            measured_at: Utc::now(),
            note: normalize_optional_field(note),
        }
    }
}
