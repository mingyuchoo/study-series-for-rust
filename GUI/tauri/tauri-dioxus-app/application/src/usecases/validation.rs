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

pub fn validate_optional_phone(phone: Option<&String>) -> Result<(), DomainError> {
    let Some(phone) = phone else {
        return Ok(());
    };

    let phone = phone.trim();
    if phone.is_empty() || is_valid_korean_mobile(phone) {
        return Ok(());
    }

    Err(DomainError::InvalidContactData(
        "전화번호는 010-1234-5678 형식의 한국 휴대폰 번호여야 합니다.".to_string(),
    ))
}

/// Strictly accepts Korean mobile numbers only: `01X-XXXX-XXXX` (11 digits) or
/// the legacy `01X-XXX-XXXX` (10 digits). Hyphens are optional, but any other
/// separator, and all landline/international numbers, are rejected.
fn is_valid_korean_mobile(phone: &str) -> bool {
    // Only digits and hyphens may appear in the input.
    if !phone.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return false;
    }

    let digits: Vec<u8> = phone.bytes().filter(u8::is_ascii_digit).collect();

    // `01` prefix + valid carrier digit + 7~8 subscriber digits (10 or 11 total).
    matches!(digits.len(), 10 | 11) && digits[0] == b'0' && digits[1] == b'1' && matches!(digits[2], b'0' | b'1' | b'6' | b'7' | b'8' | b'9')
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

    #[test]
    fn accepts_only_korean_mobile_numbers() {
        let ok = |s: &str| validate_optional_phone(Some(&s.to_string())).is_ok();

        // Empty / absent are allowed (optional field).
        assert!(validate_optional_phone(None).is_ok());
        assert!(ok("  "));

        // Valid Korean mobile numbers, with or without hyphens.
        assert!(ok("010-1234-5678"));
        assert!(ok("01012345678"));
        assert!(ok("011-123-4567")); // legacy 10-digit form

        // Rejected: alphabetic, landline, international, wrong length/prefix.
        assert!(!ok("choo"));
        assert!(!ok("02-123-4567")); // Seoul landline
        assert!(!ok("+82 10-1234-5678")); // international / spaces
        assert!(!ok("010-1234-567")); // too short
        assert!(!ok("010-1234-56789")); // too long
        assert!(!ok("021-1234-5678")); // invalid carrier digit
    }
}
