use chrono::{DateTime,
             Utc};
use serde::{Deserialize,
            Serialize};
use std::{fmt,
          str::FromStr};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Value,
    Vision,
    Kra,
    Igt,
    Kpi,
}

impl ItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            | Self::Value => "value",
            | Self::Vision => "vision",
            | Self::Kra => "kra",
            | Self::Igt => "igt",
            | Self::Kpi => "kpi",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            | Self::Value => "Value",
            | Self::Vision => "Vision",
            | Self::Kra => "KRA",
            | Self::Igt => "IGT",
            | Self::Kpi => "KPI",
        }
    }

    pub fn allowed_parent_kinds(self) -> &'static [ItemKind] {
        match self {
            | Self::Value => &[],
            | Self::Vision => &[Self::Value],
            | Self::Kra => &[Self::Vision],
            | Self::Igt => &[Self::Kra],
            | Self::Kpi => &[Self::Kra, Self::Igt],
        }
    }

    pub fn allows_parent(self, parent_kind: ItemKind) -> bool { self.allowed_parent_kinds().contains(&parent_kind) }
}

impl fmt::Display for ItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.as_str()) }
}

impl FromStr for ItemKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            | "value" => Ok(Self::Value),
            | "vision" => Ok(Self::Vision),
            | "kra" => Ok(Self::Kra),
            | "igt" => Ok(Self::Igt),
            | "kpi" => Ok(Self::Kpi),
            | _ => Err(format!("Unsupported VVKIK item kind: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    Active,
    Paused,
    Completed,
}

impl ItemStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            | Self::Active => "active",
            | Self::Paused => "paused",
            | Self::Completed => "completed",
        }
    }
}

impl fmt::Display for ItemStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.as_str()) }
}

impl FromStr for ItemStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            | "active" => Ok(Self::Active),
            | "paused" => Ok(Self::Paused),
            | "completed" => Ok(Self::Completed),
            | _ => Err(format!("Unsupported VVKIK item status: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VvkikItem {
    pub id: Uuid,
    pub kind: ItemKind,
    pub parent_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub target_value: Option<f64>,
    pub current_value: Option<f64>,
    pub unit: Option<String>,
    pub position: i64,
    pub status: ItemStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl VvkikItem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: ItemKind,
        parent_id: Option<Uuid>,
        title: String,
        description: Option<String>,
        target_value: Option<f64>,
        current_value: Option<f64>,
        unit: Option<String>,
        position: i64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            kind,
            parent_id,
            title: title.trim().to_string(),
            description: Self::normalize_optional_field(description),
            target_value,
            current_value,
            unit: Self::normalize_optional_field(unit),
            position,
            status: ItemStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        kind: Option<ItemKind>,
        parent_id: Option<Option<Uuid>>,
        title: Option<String>,
        description: Option<String>,
        target_value: Option<Option<f64>>,
        current_value: Option<Option<f64>>,
        unit: Option<String>,
        position: Option<i64>,
        status: Option<ItemStatus>,
    ) {
        if let Some(kind) = kind {
            self.kind = kind;
        }
        if let Some(parent_id) = parent_id {
            self.parent_id = parent_id;
        }
        if let Some(title) = title {
            self.title = title.trim().to_string();
        }
        if let Some(description) = description {
            self.description = Self::normalize_optional_field(Some(description));
        }
        if let Some(target_value) = target_value {
            self.target_value = target_value;
        }
        if let Some(current_value) = current_value {
            self.current_value = current_value;
        }
        if let Some(unit) = unit {
            self.unit = Self::normalize_optional_field(Some(unit));
        }
        if let Some(position) = position {
            self.position = position;
        }
        if let Some(status) = status {
            self.status = status;
        }
        self.updated_at = Utc::now();
    }

    pub fn is_kpi(&self) -> bool { self.kind == ItemKind::Kpi }

    fn normalize_optional_field(value: Option<String>) -> Option<String> { value.map(|value| value.trim().to_string()).filter(|value| !value.is_empty()) }
}

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
            note: VvkikItem::normalize_optional_field(note),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_trims_title_and_discards_blank_optional_fields() {
        let item = VvkikItem::new(
            ItemKind::Vision,
            None,
            "  Build a focused practice  ".to_string(),
            Some("  ".to_string()),
            None,
            None,
            Some("  sessions  ".to_string()),
            0,
        );

        assert_eq!(item.title, "Build a focused practice");
        assert_eq!(item.description, None);
        assert_eq!(item.unit, Some("sessions".to_string()));
        assert_eq!(item.status, ItemStatus::Active);
    }

    #[test]
    fn item_kind_knows_allowed_hierarchy() {
        assert!(ItemKind::Vision.allows_parent(ItemKind::Value));
        assert!(ItemKind::Kra.allows_parent(ItemKind::Vision));
        assert!(ItemKind::Igt.allows_parent(ItemKind::Kra));
        assert!(ItemKind::Kpi.allows_parent(ItemKind::Igt));
        assert!(!ItemKind::Value.allows_parent(ItemKind::Vision));
    }
}
