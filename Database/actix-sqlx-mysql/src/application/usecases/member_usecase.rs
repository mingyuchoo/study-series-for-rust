use crate::domain::{Member,
                    MemberRepository};
use std::sync::Arc;

/// Member 유스케이스
/// 비즈니스 로직을 조율하고 실행합니다.
/// 리포지토리 트레이트에 의존하므로 구체적인 구현에 독립적입니다.
pub struct MemberUseCase {
    repository: Arc<dyn MemberRepository>,
}

impl MemberUseCase {
    /// 새로운 MemberUseCase 인스턴스를 생성합니다.
    pub fn new(repository: Arc<dyn MemberRepository>) -> Self {
        Self {
            repository,
        }
    }

    /// 새로운 Member를 생성합니다.
    pub async fn create_member(&self, name: String) -> Result<Member, Box<dyn std::error::Error>> {
        // 이름 유효성 검증
        if name.is_empty() {
            return Err("이름은 비어있을 수 없습니다.".into());
        }
        if name.len() > 64 {
            return Err("이름은 64자를 초과할 수 없습니다.".into());
        }

        // 리포지토리를 통해 Member 생성
        let member = self.repository.create(name).await?;

        // 생성된 Member 유효성 검증
        member.validate()?;

        Ok(member)
    }

    /// ID로 Member를 조회합니다.
    pub async fn get_member_by_id(&self, id: &str) -> Result<Option<Member>, Box<dyn std::error::Error>> { self.repository.find_by_id(id).await }

    /// 모든 Member를 조회합니다.
    pub async fn get_all_members(&self) -> Result<Vec<Member>, Box<dyn std::error::Error>> { self.repository.find_all().await }

    /// Member의 총 개수를 조회합니다.
    pub async fn get_member_count(&self) -> Result<i64, Box<dyn std::error::Error>> { self.repository.count().await }

    /// Member를 업데이트합니다.
    pub async fn update_member(&self, member: Member) -> Result<(), Box<dyn std::error::Error>> {
        // 유효성 검증
        member.validate()?;

        // 리포지토리를 통해 업데이트
        self.repository.update(&member).await
    }

    /// ID로 Member를 삭제합니다.
    pub async fn delete_member(&self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // ID 유효성 검증
        if id.is_empty() {
            return Err("ID는 비어있을 수 없습니다.".into());
        }

        // 리포지토리를 통해 삭제
        self.repository.delete(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    // 테스트용 Mock 리포지토리
    struct MockMemberRepository;

    #[async_trait]
    impl MemberRepository for MockMemberRepository {
        async fn create(&self, name: String) -> Result<Member, Box<dyn std::error::Error>> { Ok(Member::new("MEMB0001".to_string(), name)) }

        async fn find_by_id(&self, _id: &str) -> Result<Option<Member>, Box<dyn std::error::Error>> {
            Ok(Some(Member::new("MEMB0001".to_string(), "홍길동".to_string())))
        }

        async fn find_all(&self) -> Result<Vec<Member>, Box<dyn std::error::Error>> { Ok(vec![Member::new("MEMB0001".to_string(), "홍길동".to_string())]) }

        async fn count(&self) -> Result<i64, Box<dyn std::error::Error>> { Ok(1) }

        async fn update(&self, _member: &Member) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }

        async fn delete(&self, _id: &str) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
    }

    #[tokio::test]
    async fn test_create_member() {
        let repository = Arc::new(MockMemberRepository);
        let usecase = MemberUseCase::new(repository);

        let result = usecase.create_member("홍길동".to_string()).await;
        assert!(result.is_ok());

        let member = result.unwrap();
        assert_eq!(member.name, "홍길동");
    }

    #[tokio::test]
    async fn test_create_member_with_empty_name() {
        let repository = Arc::new(MockMemberRepository);
        let usecase = MemberUseCase::new(repository);

        let result = usecase.create_member("".to_string()).await;
        assert!(result.is_err());
    }
}
