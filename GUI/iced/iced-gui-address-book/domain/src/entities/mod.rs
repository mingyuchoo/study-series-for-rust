use serde::{Deserialize,
            Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub id: Option<i64>,
    pub name: String,
    pub phone: String,
    pub email: String,
    pub address: String,
}

impl Address {
    pub fn new(name: String, phone: String, email: String, address: String) -> Self {
        Self {
            id: None,
            name,
            phone,
            email,
            address,
        }
    }
}
