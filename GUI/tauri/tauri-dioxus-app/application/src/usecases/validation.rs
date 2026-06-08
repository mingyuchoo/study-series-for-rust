use domain::DomainError;

pub fn validate_name(name: &str) -> Result<(), DomainError> {
    if name.trim().is_empty() {
        return Err(DomainError::InvalidContactData("Name cannot be empty".to_string()));
    }

    Ok(())
}

pub fn validate_optional_email(email: Option<&String>) -> Result<(), DomainError> {
    let Some(email) = email else {
        return Ok(());
    };

    let email = email.trim();
    if email.is_empty() || is_valid_email(email) {
        return Ok(());
    }

    Err(DomainError::InvalidContactData("Email format is invalid".to_string()))
}

fn is_valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };

    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_basic_email_shape() {
        assert!(validate_optional_email(Some(&"ada@example.com".to_string())).is_ok());
        assert!(validate_optional_email(Some(&"  ".to_string())).is_ok());
        assert!(validate_optional_email(None).is_ok());
        assert!(validate_optional_email(Some(&"ada.example.com".to_string())).is_err());
        assert!(validate_optional_email(Some(&"ada@example".to_string())).is_err());
    }
}
