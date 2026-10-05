use crate::error::ValidationError;
use serde::{Deserialize,
            Serialize};

/// 주소록 항목 한 건을 나타내는 도메인 엔티티.
///
/// `id` 는 영속화되기 전에는 `None` 이며, 저장 후 백엔드가 부여한 식별자가
/// 채워진다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    pub id: Option<i64>,
    pub name: String,
    pub phone: String,
    pub email: String,
    pub address: String,
}

impl Address {
    /// 검증을 거쳐 새 (아직 영속화되지 않은) [`Address`] 를 만든다.
    ///
    /// 이름·전화번호는 비어 있을 수 없고, 이메일은 최소한 `'@'` 를 포함해야
    /// 한다. 이 생성자는 "신규 입력" 경로에서만 쓰인다. 이미 저장된
    /// 데이터를 다시 읽어올 때는 신뢰된 데이터로 간주하여 struct 리터럴로
    /// 직접 재구성한다(검증하지 않는다).
    ///
    /// # Errors
    /// 입력이 불변식을 위반하면 [`ValidationError`] 를 반환한다.
    pub fn new(name: String, phone: String, email: String, address: String) -> Result<Self, ValidationError> {
        if name.trim().is_empty() {
            return Err(ValidationError::EmptyName);
        }
        if phone.trim().is_empty() {
            return Err(ValidationError::EmptyPhone);
        }
        if !email.contains('@') {
            return Err(ValidationError::InvalidEmail(email));
        }

        Ok(Self {
            id: None,
            name,
            phone,
            email,
            address,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_accepts_valid_input() {
        let addr = Address::new("Alice".into(), "010-1234-5678".into(), "alice@example.com".into(), "Seoul".into());
        let addr = addr.expect("valid input should be accepted");
        assert_eq!(addr.id, None);
        assert_eq!(addr.name, "Alice");
    }

    #[test]
    fn new_rejects_empty_name() {
        let addr = Address::new("   ".into(), "010".into(), "a@b.com".into(), "Seoul".into());
        assert_eq!(addr, Err(ValidationError::EmptyName));
    }

    #[test]
    fn new_rejects_empty_phone() {
        let addr = Address::new("Alice".into(), "".into(), "a@b.com".into(), "Seoul".into());
        assert_eq!(addr, Err(ValidationError::EmptyPhone));
    }

    #[test]
    fn new_rejects_email_without_at_sign() {
        let addr = Address::new("Alice".into(), "010".into(), "not-an-email".into(), "Seoul".into());
        assert_eq!(addr, Err(ValidationError::InvalidEmail("not-an-email".into())));
    }
}
