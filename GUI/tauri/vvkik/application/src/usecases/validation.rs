use domain::{DomainError,
             ItemKind,
             VvkikItem};

pub fn validate_title(title: &str) -> Result<(), DomainError> {
    if title.trim().is_empty() {
        return Err(DomainError::InvalidVvkikData("제목을 입력하세요.".to_string()));
    }

    Ok(())
}

pub fn validate_parent(kind: ItemKind, parent: Option<&VvkikItem>) -> Result<(), DomainError> {
    match (kind, parent) {
        | (ItemKind::Value, Some(_)) => Err(DomainError::InvalidVvkikData("Value는 최상위 항목이어야 합니다.".to_string())),
        | (ItemKind::Value, None) => Ok(()),
        | (_, None) => Err(DomainError::InvalidVvkikData(format!("{} 항목의 상위 항목을 선택하세요.", kind.label()))),
        | (_, Some(parent)) if kind.allows_parent(parent.kind) => Ok(()),
        | (_, Some(parent)) => Err(DomainError::InvalidVvkikData(format!(
            "{} 항목은 {} 아래에 둘 수 없습니다.",
            kind.label(),
            parent.kind.label()
        ))),
    }
}

pub fn validate_kpi_values(kind: ItemKind, target_value: Option<f64>, current_value: Option<f64>, unit: Option<&str>) -> Result<(), DomainError> {
    if kind != ItemKind::Kpi && (target_value.is_some() || current_value.is_some() || unit.is_some_and(|unit| !unit.trim().is_empty())) {
        return Err(DomainError::InvalidVvkikData("목표값, 현재값, 단위는 KPI 항목에서만 사용합니다.".to_string()));
    }

    Ok(())
}

pub fn validate_measurement_value(value: f64) -> Result<(), DomainError> {
    if !value.is_finite() {
        return Err(DomainError::InvalidVvkikData("KPI 측정값은 유효한 숫자여야 합니다.".to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_required_title() {
        assert!(validate_title("Build income engine").is_ok());
        assert!(validate_title("   ").is_err());
    }

    #[test]
    fn validates_parent_hierarchy() {
        let value = VvkikItem::new(domain::NewVvkikItem {
            kind: ItemKind::Value,
            parent_id: None,
            title: "Freedom".to_string(),
            description: None,
            target_value: None,
            current_value: None,
            unit: None,
            position: 0,
            aggregation: domain::KpiAggregation::default(),
        });
        assert!(validate_parent(ItemKind::Vision, Some(&value)).is_ok());
        assert!(validate_parent(ItemKind::Kra, Some(&value)).is_err());
        assert!(validate_parent(ItemKind::Value, Some(&value)).is_err());
    }
}
