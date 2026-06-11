use serde::{Deserialize,
            Serialize};
use std::{fmt,
          str::FromStr};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Identity,
    Vision,
    Kra,
    Igt,
    Kpi,
}

impl ItemKind {
    /// IVKIK 계층 순서. 트리 정렬과 탭 순서의 기준이 된다.
    pub const ALL: [ItemKind; 5] = [Self::Identity, Self::Vision, Self::Kra, Self::Igt, Self::Kpi];

    pub fn as_str(self) -> &'static str {
        match self {
            | Self::Identity => "identity",
            | Self::Vision => "vision",
            | Self::Kra => "kra",
            | Self::Igt => "igt",
            | Self::Kpi => "kpi",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            | Self::Identity => "Identity",
            | Self::Vision => "Vision",
            | Self::Kra => "KRA",
            | Self::Igt => "IGT",
            | Self::Kpi => "KPI",
        }
    }

    /// 계층에서 이 단계가 차지하는 순위(0 = 최상위).
    pub fn rank(self) -> usize { Self::ALL.iter().position(|kind| *kind == self).expect("ALL covers every kind") }

    /// 부모는 항상 정확히 1단계 위의 단계만 허용한다.
    pub fn allowed_parent_kinds(self) -> &'static [ItemKind] {
        match self {
            | Self::Identity => &[],
            | Self::Vision => &[Self::Identity],
            | Self::Kra => &[Self::Vision],
            | Self::Igt => &[Self::Kra],
            | Self::Kpi => &[Self::Igt],
        }
    }

    pub fn allowed_child_kinds(self) -> &'static [ItemKind] {
        match self {
            | Self::Identity => &[Self::Vision],
            | Self::Vision => &[Self::Kra],
            | Self::Kra => &[Self::Igt],
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
            | "identity" => Ok(Self::Identity),
            | "vision" => Ok(Self::Vision),
            | "kra" => Ok(Self::Kra),
            | "igt" => Ok(Self::Igt),
            | "kpi" => Ok(Self::Kpi),
            | _ => Err(format!("Unsupported IVKIK item kind: {value}")),
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
            | _ => Err(format!("Unsupported IVKIK item status: {value}")),
        }
    }
}

/// KPI 측정 기록들을 현재값으로 취합하는 방식.
///
/// 체지방률처럼 "지금 수준"이 의미 있는 지표는 `Latest`, 월 커밋 수처럼
/// 기록이 쌓여 성과가 되는 지표는 `Sum`, 평균 수면 시간처럼 기록의
/// 평균이 수준을 나타내는 지표는 `Average`를 쓴다.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KpiAggregation {
    #[default]
    Latest,
    Sum,
    Average,
}

impl KpiAggregation {
    pub const ALL: [KpiAggregation; 3] = [Self::Latest, Self::Sum, Self::Average];

    pub fn as_str(self) -> &'static str {
        match self {
            | Self::Latest => "latest",
            | Self::Sum => "sum",
            | Self::Average => "average",
        }
    }

    /// 측정값들로부터 현재값을 계산한다. `values`는 최신 기록이 앞에
    /// 오도록(측정 시각 내림차순) 정렬되어 있어야 한다. 기록이 없으면
    /// `None`을 돌려준다.
    pub fn aggregate(self, values: &[f64]) -> Option<f64> {
        if values.is_empty() {
            return None;
        }

        match self {
            | Self::Latest => values.first().copied(),
            | Self::Sum => Some(values.iter().sum()),
            | Self::Average => Some(values.iter().sum::<f64>() / values.len() as f64),
        }
    }
}

impl fmt::Display for KpiAggregation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.as_str()) }
}

impl FromStr for KpiAggregation {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            | "latest" => Ok(Self::Latest),
            | "sum" => Ok(Self::Sum),
            | "average" => Ok(Self::Average),
            | _ => Err(format!("Unsupported KPI aggregation: {value}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_serializes_to_snake_case_wire_format() {
        assert_eq!(serde_json::to_string(&ItemKind::Kra).unwrap(), "\"kra\"");
        assert_eq!(serde_json::from_str::<ItemKind>("\"identity\"").unwrap(), ItemKind::Identity);
    }

    #[test]
    fn kind_knows_allowed_hierarchy() {
        assert!(ItemKind::Vision.allows_parent(ItemKind::Identity));
        assert!(ItemKind::Kpi.allows_parent(ItemKind::Igt));
        assert!(!ItemKind::Kpi.allows_parent(ItemKind::Kra), "KPI는 정확히 1단계 위인 IGT만 허용");
        assert!(!ItemKind::Identity.allows_parent(ItemKind::Vision));
    }

    #[test]
    fn every_kind_has_exactly_one_parent_kind_except_identity() {
        for kind in ItemKind::ALL {
            let parents = kind.allowed_parent_kinds();
            if kind == ItemKind::Identity {
                assert!(parents.is_empty());
            } else {
                assert_eq!(parents.len(), 1, "{kind}의 부모는 정확히 1종류");
                assert_eq!(parents[0].rank() + 1, kind.rank(), "{kind}의 부모는 정확히 1단계 위");
            }
        }
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
        assert_eq!(ItemKind::Identity.rank(), 0);
        assert_eq!(ItemKind::Kpi.rank(), 4);
    }

    #[test]
    fn aggregation_serializes_to_snake_case_wire_format() {
        assert_eq!(serde_json::to_string(&KpiAggregation::Latest).unwrap(), "\"latest\"");
        assert_eq!(serde_json::from_str::<KpiAggregation>("\"sum\"").unwrap(), KpiAggregation::Sum);
        assert_eq!("average".parse::<KpiAggregation>().unwrap(), KpiAggregation::Average);
        assert!("unknown".parse::<KpiAggregation>().is_err());
    }

    #[test]
    fn aggregation_computes_current_value_from_newest_first_values() {
        // 최신 기록(40.0)이 맨 앞에 오는 내림차순 입력.
        let values = [40.0, 30.0, 20.0];

        assert_eq!(KpiAggregation::Latest.aggregate(&values), Some(40.0));
        assert_eq!(KpiAggregation::Sum.aggregate(&values), Some(90.0));
        assert_eq!(KpiAggregation::Average.aggregate(&values), Some(30.0));
        assert_eq!(KpiAggregation::Sum.aggregate(&[]), None);
    }
}
