use serde::{Deserialize,
            Serialize};
use std::{fmt,
          str::FromStr};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Value,
    Vision,
    Kra,
    Igt,
    Kpi,
}

impl ItemKind {
    /// VVKIK 계층 순서. 트리 정렬과 탭 순서의 기준이 된다.
    pub const ALL: [ItemKind; 5] = [Self::Value, Self::Vision, Self::Kra, Self::Igt, Self::Kpi];

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

    /// 계층에서 이 단계가 차지하는 순위(0 = 최상위).
    pub fn rank(self) -> usize {
        Self::ALL.iter().position(|kind| *kind == self).expect("ALL covers every kind")
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

    pub fn allowed_child_kinds(self) -> &'static [ItemKind] {
        match self {
            | Self::Value => &[Self::Vision],
            | Self::Vision => &[Self::Kra],
            | Self::Kra => &[Self::Igt, Self::Kpi],
            | Self::Igt => &[Self::Kpi],
            | Self::Kpi => &[],
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
    pub const ALL: [ItemStatus; 3] = [Self::Active, Self::Paused, Self::Completed];

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_serializes_to_snake_case_wire_format() {
        assert_eq!(serde_json::to_string(&ItemKind::Kra).unwrap(), "\"kra\"");
        assert_eq!(serde_json::from_str::<ItemKind>("\"value\"").unwrap(), ItemKind::Value);
    }

    #[test]
    fn kind_knows_allowed_hierarchy() {
        assert!(ItemKind::Vision.allows_parent(ItemKind::Value));
        assert!(ItemKind::Kpi.allows_parent(ItemKind::Igt));
        assert!(ItemKind::Kpi.allows_parent(ItemKind::Kra));
        assert!(!ItemKind::Value.allows_parent(ItemKind::Vision));
    }

    #[test]
    fn child_kinds_mirror_parent_kinds() {
        for parent in ItemKind::ALL {
            for child in parent.allowed_child_kinds() {
                assert!(child.allows_parent(parent), "{child}는 {parent}의 하위여야 한다");
            }
        }
    }

    #[test]
    fn rank_follows_all_order() {
        assert_eq!(ItemKind::Value.rank(), 0);
        assert_eq!(ItemKind::Kpi.rank(), 4);
    }
}
