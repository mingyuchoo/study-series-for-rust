use chrono::NaiveDate;
use domain::{DomainError,
             IkikItem,
             ItemKind,
             ValidationIssue};

pub fn validate_title(title: &str) -> Result<(), DomainError> {
    if title.trim().is_empty() {
        return Err(DomainError::Validation(ValidationIssue::TitleRequired));
    }

    Ok(())
}

pub fn validate_parent(kind: ItemKind, parent: Option<&IkikItem>) -> Result<(), DomainError> {
    match (kind, parent) {
        | (ItemKind::Identity, Some(_)) => Err(DomainError::Validation(ValidationIssue::IdentityMustBeRoot)),
        | (ItemKind::Identity, None) => Ok(()),
        | (_, None) => Err(DomainError::Validation(ValidationIssue::ParentRequired {
            kind,
        })),
        | (_, Some(parent)) if kind.allows_parent(parent.kind) => Ok(()),
        | (_, Some(parent)) => Err(DomainError::Validation(ValidationIssue::ParentKindMismatch {
            kind,
            parent_kind: parent.kind,
        })),
    }
}

pub fn validate_kpi_values(kind: ItemKind, target_value: Option<f64>, current_value: Option<f64>, unit: Option<&str>) -> Result<(), DomainError> {
    if kind != ItemKind::Kpi && (target_value.is_some() || current_value.is_some() || unit.is_some_and(|unit| !unit.trim().is_empty())) {
        return Err(DomainError::Validation(ValidationIssue::KpiFieldsOnNonKpi));
    }

    Ok(())
}

/// Identity는 마감 없이 지속되는 단계이므로 마감 기한을 허용하지 않는다.
pub fn validate_due_date(kind: ItemKind, due_date: Option<NaiveDate>) -> Result<(), DomainError> {
    if kind == ItemKind::Identity && due_date.is_some() {
        return Err(DomainError::Validation(ValidationIssue::DueDateOnIdentity));
    }

    Ok(())
}

pub fn validate_measurement_value(value: f64) -> Result<(), DomainError> {
    if !value.is_finite() {
        return Err(DomainError::Validation(ValidationIssue::MeasurementNotNumeric));
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
        let value = IkikItem::new(domain::NewIkikItem {
            kind: ItemKind::Identity,
            parent_id: None,
            title: "Freedom".to_string(),
            description: None,
            target_value: None,
            current_value: None,
            unit: None,
            position: 0,
            aggregation: domain::KpiAggregation::default(),
            due_date: None,
        });
        assert!(validate_parent(ItemKind::Kra, Some(&value)).is_ok());
        assert!(validate_parent(ItemKind::Igt, Some(&value)).is_err());
        assert!(validate_parent(ItemKind::Identity, Some(&value)).is_err());
    }

    #[test]
    fn validates_due_date_by_kind() {
        let due = NaiveDate::from_ymd_opt(2026, 6, 30);
        assert!(validate_due_date(ItemKind::Identity, due).is_err());
        assert!(validate_due_date(ItemKind::Identity, None).is_ok());
        assert!(validate_due_date(ItemKind::Kra, due).is_ok());
        assert!(validate_due_date(ItemKind::Igt, due).is_ok());
        assert!(validate_due_date(ItemKind::Kpi, due).is_ok());
    }
}
