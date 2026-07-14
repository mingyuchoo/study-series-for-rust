use crate::error::AppError;
use domain::{entities::Address,
             repositories::AddressRepository};
use std::sync::Arc;

/// 주소록 use case 모음.
///
/// 저장소를 `Arc<dyn AddressRepository>` 로 주입받아 구체 구현(SQLite 등)과
/// 분리된다. 검증·오케스트레이션 같은 애플리케이션 규칙이 들어갈 자리이며,
/// 현재는 입력 검증 ([`Address::new`])과 저장소 위임을 담당한다.
pub struct AddressUseCases {
    repository: Arc<dyn AddressRepository>,
}

impl AddressUseCases {
    /// 주입된 저장소로 use case 모음을 만든다.
    pub fn new(repository: Arc<dyn AddressRepository>) -> Self {
        Self {
            repository,
        }
    }

    /// 입력을 검증한 뒤 새 주소를 저장한다.
    ///
    /// # Errors
    /// 검증 실패 시 [`AppError::Validation`], 저장 실패 시
    /// [`AppError::Repository`].
    pub fn create_address(&self, name: String, phone: String, email: String, address: String) -> Result<Address, AppError> {
        let new_address = Address::new(name, phone, email, address)?;
        Ok(self.repository.create(new_address)?)
    }

    /// `id` 로 주소를 조회한다.
    pub fn get_address(&self, id: i64) -> Result<Option<Address>, AppError> { Ok(self.repository.read(id)?) }

    /// 저장된 모든 주소를 조회한다.
    pub fn get_all_addresses(&self) -> Result<Vec<Address>, AppError> { Ok(self.repository.read_all()?) }

    /// 기존 주소를 갱신한다.
    pub fn update_address(&self, address: Address) -> Result<Address, AppError> { Ok(self.repository.update(address)?) }

    /// `id` 에 해당하는 주소를 삭제한다.
    pub fn delete_address(&self, id: i64) -> Result<(), AppError> { Ok(self.repository.delete(id)?) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::error::{RepositoryError,
                        ValidationError};
    use std::{collections::HashMap,
              sync::Mutex};

    /// 저장소 trait 의 인메모리 fake 구현.
    ///
    /// DB 없이 use case 를 검증할 수 있음을 보여주며, 이는 도메인이 저장 기술과
    /// 분리(low coupling)되어 있다는 증거다.
    #[derive(Default)]
    struct FakeRepository {
        items: Mutex<HashMap<i64, Address>>,
        next_id: Mutex<i64>,
    }

    impl AddressRepository for FakeRepository {
        fn create(&self, mut address: Address) -> Result<Address, RepositoryError> {
            let mut next = self.next_id.lock().unwrap();
            *next += 1;
            address.id = Some(*next);
            self.items.lock().unwrap().insert(*next, address.clone());
            Ok(address)
        }

        fn read(&self, id: i64) -> Result<Option<Address>, RepositoryError> { Ok(self.items.lock().unwrap().get(&id).cloned()) }

        fn read_all(&self) -> Result<Vec<Address>, RepositoryError> { Ok(self.items.lock().unwrap().values().cloned().collect()) }

        fn update(&self, address: Address) -> Result<Address, RepositoryError> {
            let id = address.id.ok_or(RepositoryError::MissingId)?;
            self.items.lock().unwrap().insert(id, address.clone());
            Ok(address)
        }

        fn delete(&self, id: i64) -> Result<(), RepositoryError> {
            self.items.lock().unwrap().remove(&id);
            Ok(())
        }
    }

    fn usecases() -> AddressUseCases { AddressUseCases::new(Arc::new(FakeRepository::default())) }

    #[test]
    fn create_then_read_all_roundtrips() {
        let uc = usecases();
        let created = uc
            .create_address("Alice".into(), "010".into(), "alice@example.com".into(), "Seoul".into())
            .expect("create should succeed");
        assert_eq!(created.id, Some(1));

        let all = uc.get_all_addresses().expect("read_all should succeed");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Alice");
    }

    #[test]
    fn create_surfaces_validation_error() {
        let uc = usecases();
        let result = uc.create_address("".into(), "010".into(), "a@b.com".into(), "Seoul".into());
        assert_eq!(result, Err(AppError::Validation(ValidationError::EmptyName)));
    }

    #[test]
    fn update_and_delete() {
        let uc = usecases();
        let mut created = uc
            .create_address("Bob".into(), "010".into(), "bob@example.com".into(), "Busan".into())
            .expect("create should succeed");

        created.name = "Bobby".into();
        uc.update_address(created.clone()).expect("update should succeed");
        assert_eq!(uc.get_address(created.id.unwrap()).unwrap().unwrap().name, "Bobby");

        uc.delete_address(created.id.unwrap()).expect("delete should succeed");
        assert!(uc.get_all_addresses().unwrap().is_empty());
    }

    struct FailingRepository;

    impl AddressRepository for FailingRepository {
        fn create(&self, _address: Address) -> Result<Address, RepositoryError> { Err(RepositoryError::Backend("create failed".into())) }

        fn read(&self, _id: i64) -> Result<Option<Address>, RepositoryError> { Err(RepositoryError::Backend("read failed".into())) }

        fn read_all(&self) -> Result<Vec<Address>, RepositoryError> { Err(RepositoryError::Backend("read all failed".into())) }

        fn update(&self, _address: Address) -> Result<Address, RepositoryError> { Err(RepositoryError::Backend("update failed".into())) }

        fn delete(&self, _id: i64) -> Result<(), RepositoryError> { Err(RepositoryError::Backend("delete failed".into())) }
    }

    #[test]
    fn repository_errors_are_propagated_by_every_usecase() {
        let uc = AddressUseCases::new(Arc::new(FailingRepository));
        let address = Address {
            id: Some(1),
            name: "Alice".into(),
            phone: "010".into(),
            email: "alice@example.com".into(),
            address: "Seoul".into(),
        };

        assert!(matches!(
            uc.create_address(address.name.clone(), address.phone.clone(), address.email.clone(), address.address.clone()),
            Err(AppError::Repository(_))
        ));
        assert!(matches!(uc.get_address(1), Err(AppError::Repository(_))));
        assert!(matches!(uc.get_all_addresses(), Err(AppError::Repository(_))));
        assert!(matches!(uc.update_address(address), Err(AppError::Repository(_))));
        assert!(matches!(uc.delete_address(1), Err(AppError::Repository(_))));
    }
}
