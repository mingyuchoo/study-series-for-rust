use domain::Contact;
use serde::{Deserialize,
            Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ContactDto {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub memo: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateContactRequest {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub memo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateContactRequest {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub memo: Option<String>,
}

impl From<Contact> for ContactDto {
    fn from(contact: Contact) -> Self {
        Self {
            id: contact.id.to_string(),
            name: contact.name,
            email: contact.email,
            phone: contact.phone,
            memo: contact.memo,
            created_at: contact.created_at.to_rfc3339(),
            updated_at: contact.updated_at.to_rfc3339(),
        }
    }
}
