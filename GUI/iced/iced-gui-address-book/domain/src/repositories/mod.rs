use crate::entities::Address;

pub trait AddressRepository: Send + Sync {
    fn create(&self, address: Address) -> Result<Address, String>;
    fn read(&self, id: i64) -> Result<Option<Address>, String>;
    fn read_all(&self) -> Result<Vec<Address>, String>;
    fn update(&self, address: Address) -> Result<Address, String>;
    fn delete(&self, id: i64) -> Result<(), String>;
}
