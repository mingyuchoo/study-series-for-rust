use domain::{entities::Address,
             repositories::AddressRepository};
use std::sync::Arc;

pub struct AddressUseCases {
    repository: Arc<dyn AddressRepository>,
}

impl AddressUseCases {
    pub fn new(repository: Arc<dyn AddressRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub fn create_address(&self, name: String, phone: String, email: String, address: String) -> Result<Address, String> {
        let new_address = Address::new(name, phone, email, address);
        self.repository.create(new_address)
    }

    pub fn get_address(&self, id: i64) -> Result<Option<Address>, String> { self.repository.read(id) }

    pub fn get_all_addresses(&self) -> Result<Vec<Address>, String> { self.repository.read_all() }

    pub fn update_address(&self, address: Address) -> Result<Address, String> { self.repository.update(address) }

    pub fn delete_address(&self, id: i64) -> Result<(), String> { self.repository.delete(id) }
}
