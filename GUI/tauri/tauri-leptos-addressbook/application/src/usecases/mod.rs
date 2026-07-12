use domain::{entities::Address,
             repositories::AddressRepository};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct AddressService {
    repository: Box<dyn AddressRepository>,
}

impl AddressService {
    pub fn new(repository: Box<dyn AddressRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn create_address(&self, name: String, phone: String, email: String) -> Result<Address> {
        let address = Address::new(name, phone, email);
        self.repository.create(address).await
    }

    pub async fn get_address(&self, id: Uuid) -> Result<Option<Address>> { self.repository.get_by_id(id).await }

    pub async fn get_all_addresses(&self) -> Result<Vec<Address>> { self.repository.get_all().await }

    pub async fn update_address(&self, address: Address) -> Result<Address> { self.repository.update(address).await }

    pub async fn delete_address(&self, id: Uuid) -> Result<bool> { self.repository.delete(id).await }
}
