use crate::{entities::Address,
            error::RepositoryError};

/// [`Address`] 영속화를 위한 추상 인터페이스.
///
/// 도메인이 구체적인 저장 기술과 분리(low coupling)되도록 trait 으로 정의한다.
/// 구현체는 infrastructure 레이어가 제공한다. `Send + Sync` 는 GUI 의 비동기
/// 태스크와 스레드 간 공유(`Arc<dyn AddressRepository>`)를 위해 필요하다.
pub trait AddressRepository: Send + Sync {
    /// 새 주소를 저장하고, 부여된 `id` 가 채워진 주소를 돌려준다.
    fn create(&self, address: Address) -> Result<Address, RepositoryError>;

    /// `id` 로 주소를 조회한다. 없으면 `Ok(None)`.
    fn read(&self, id: i64) -> Result<Option<Address>, RepositoryError>;

    /// 저장된 모든 주소를 조회한다.
    fn read_all(&self) -> Result<Vec<Address>, RepositoryError>;

    /// 기존 주소를 갱신한다. `address.id` 가 `None` 이면
    /// [`RepositoryError::MissingId`].
    fn update(&self, address: Address) -> Result<Address, RepositoryError>;

    /// `id` 에 해당하는 주소를 삭제한다.
    fn delete(&self, id: i64) -> Result<(), RepositoryError>;
}
