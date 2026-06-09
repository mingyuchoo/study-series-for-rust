use serde::{Deserialize,
            Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VvkikItem {
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

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KpiMeasurement {
    pub id: String,
    pub kpi_id: String,
    pub value: f64,
    pub measured_at: String,
    pub note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct RecordKpiMeasurementRequest {
    pub kpi_id: String,
    pub value: f64,
    pub note: Option<String>,
}

/// Structured error returned by Tauri commands, mirroring the backend
/// `ApiError`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiError {
    pub kind: ApiErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorKind {
    NotFound,
    Validation,
    Database,
    Internal,
}

pub fn kind_label(kind: &str) -> &'static str {
    match kind {
        | "value" => "Value",
        | "vision" => "Vision",
        | "kra" => "KRA",
        | "igt" => "IGT",
        | "kpi" => "KPI",
        | _ => "Item",
    }
}

pub fn kind_description(kind: &str) -> &'static str {
    match kind {
        | "value" => "무엇을 중요하게 여길 것인가?",
        | "vision" => "어디로 갈 것인가?",
        | "kra" => "반드시 집중해야 할 핵심 영역",
        | "igt" => "실제 돈과 성과를 만드는 실행 업무",
        | "kpi" => "측정과 피드백으로 시스템을 조정하는 지표",
        | _ => "",
    }
}

pub fn allowed_parent_kinds(kind: &str) -> &'static [&'static str] {
    match kind {
        | "value" => &[],
        | "vision" => &["value"],
        | "kra" => &["vision"],
        | "igt" => &["kra"],
        | "kpi" => &["kra", "igt"],
        | _ => &[],
    }
}
