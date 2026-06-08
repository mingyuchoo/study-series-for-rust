use super::validation::{validate_name,
                        validate_optional_email};
use domain::{Contact,
             ContactRepository,
             DomainError};
use std::sync::Arc;
use uuid::Uuid;

pub struct UpdateContactUseCase {
    repository: Arc<dyn ContactRepository>,
}

impl UpdateContactUseCase {
    pub fn new(repository: Arc<dyn ContactRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(
        &self,
        id: Uuid,
        name: Option<String>,
        email: Option<String>,
        phone: Option<String>,
        address: Option<String>,
    ) -> Result<Contact, DomainError> {
        if let Some(name) = name.as_ref() {
            validate_name(name)?;
        }
        validate_optional_email(email.as_ref())?;

        let mut contact = self.repository.get_by_id(id).await?.ok_or(DomainError::ContactNotFound)?;

        contact.update(name, email, phone, address);
        self.repository.update(contact).await
    }
}
