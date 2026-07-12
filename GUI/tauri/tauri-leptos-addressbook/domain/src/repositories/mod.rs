use crate::entities::Address;
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[async_trait::async_trait]
pub trait AddressRepository: Send + Sync {
    async fn create(&self, address: Address) -> Result<Address>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Address>>;
    async fn get_all(&self) -> Result<Vec<Address>>;
    async fn update(&self, address: Address) -> Result<Address>;
    async fn delete(&self, id: Uuid) -> Result<bool>;
}
