use crate::{ItemKind,
            ItemStatus,
            KpiAggregation};
use serde::{Deserialize,
            Serialize};

/// 화면과 커맨드 경계를 오가는 VVKIK 항목 표현.
/// id는 UUID 문자열, 시각은 RFC3339 문자열로 고정한다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VvkikItemDto {
    pub id: String,
    pub kind: ItemKind,
    pub parent_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub target_value: Option<f64>,
    pub current_value: Option<f64>,
    pub unit: Option<String>,
    pub position: i64,
    pub status: ItemStatus,
    #[serde(default)]
    pub aggregation: KpiAggregation,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateItemRequest {
    pub kind: ItemKind,
    pub parent_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub target_value: Option<f64>,
    pub current_value: Option<f64>,
    pub unit: Option<String>,
    pub position: Option<i64>,
    #[serde(default)]
    pub aggregation: KpiAggregation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateItemRequest {
    pub id: String,
    pub kind: Option<ItemKind>,
    pub parent_id: Option<Option<String>>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub target_value: Option<Option<f64>>,
    pub current_value: Option<Option<f64>>,
    pub unit: Option<String>,
    pub position: Option<i64>,
    pub status: Option<ItemStatus>,
    #[serde(default)]
    pub aggregation: Option<KpiAggregation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KpiMeasurementDto {
    pub id: String,
    pub kpi_id: String,
    pub value: f64,
    pub measured_at: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordKpiMeasurementRequest {
    pub kpi_id: String,
    pub value: f64,
    pub note: Option<String>,
}

/// 항목 정의 변경 이력 한 건. `field`는 와이어 필드 이름(kind, parent,
/// title, description, target_value, unit, status, aggregation)이고,
/// 화면 레이블은 프런트엔드가 매긴다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemRevisionDto {
    pub id: String,
    pub item_id: String,
    pub field: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub changed_at: String,
}
