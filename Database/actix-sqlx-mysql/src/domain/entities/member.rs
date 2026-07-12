use serde::{Deserialize,
            Serialize};

/// Member 도메인 엔티티
/// 비즈니스 로직의 핵심 개념을 표현하며 외부 프레임워크에 독립적입니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub name: String,
}

impl Member {
    /// 새로운 Member 인스턴스를 생성합니다.
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
        }
    }

    /// ID의 유효성을 검증합니다.
    pub fn validate_id(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("ID는 비어있을 수 없습니다.".to_string());
        }
        if self.id.len() != 8 {
            return Err("ID는 8자리여야 합니다.".to_string());
        }
        Ok(())
    }

    /// 이름의 유효성을 검증합니다.
    pub fn validate_name(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("이름은 비어있을 수 없습니다.".to_string());
        }
        if self.name.len() > 64 {
            return Err("이름은 64자를 초과할 수 없습니다.".to_string());
        }
        Ok(())
    }

    /// 전체 유효성을 검증합니다.
    pub fn validate(&self) -> Result<(), String> {
        self.validate_id()?;
        self.validate_name()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_member_creation() {
        let member = Member::new("MEMB0001".to_string(), "홍길동".to_string());
        assert_eq!(member.id, "MEMB0001");
        assert_eq!(member.name, "홍길동");
    }

    #[test]
    fn test_validate_id() {
        let member = Member::new("MEMB0001".to_string(), "홍길동".to_string());
        assert!(member.validate_id().is_ok());

        let invalid_member = Member::new("".to_string(), "홍길동".to_string());
        assert!(invalid_member.validate_id().is_err());
    }

    #[test]
    fn test_validate_name() {
        let member = Member::new("MEMB0001".to_string(), "홍길동".to_string());
        assert!(member.validate_name().is_ok());

        let invalid_member = Member::new("MEMB0001".to_string(), "".to_string());
        assert!(invalid_member.validate_name().is_err());
    }
}
