use serde::{Deserialize,
            Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Address {
    pub id: Uuid,
    pub name: String,
    pub phone: String,
    pub email: String,
}

impl Address {
    pub fn new(name: String, phone: String, email: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            phone,
            email,
        }
    }
}
