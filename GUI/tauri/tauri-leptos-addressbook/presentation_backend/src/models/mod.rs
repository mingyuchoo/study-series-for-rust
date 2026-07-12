use serde::{Deserialize,
            Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAddressRequest {
    pub name: String,
    pub phone: String,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateAddressRequest {
    pub id: Uuid,
    pub name: String,
    pub phone: String,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddressResponse {
    pub id: Uuid,
    pub name: String,
    pub phone: String,
    pub email: String,
}

impl From<domain::entities::Address> for AddressResponse {
    fn from(address: domain::entities::Address) -> Self {
        Self {
            id: address.id,
            name: address.name,
            phone: address.phone,
            email: address.email,
        }
    }
}
