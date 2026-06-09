use crate::{entities::Contact,
            errors::DomainError};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait ContactRepository: Send + Sync {
    async fn create(&self, contact: Contact) -> Result<Contact, DomainError>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Contact>, DomainError>;
    async fn get_all(&self) -> Result<Vec<Contact>, DomainError>;
    async fn update(&self, contact: Contact) -> Result<Contact, DomainError>;
    async fn delete(&self, id: Uuid) -> Result<(), DomainError>;
    async fn search(&self, query: &str) -> Result<Vec<Contact>, DomainError>;
}
