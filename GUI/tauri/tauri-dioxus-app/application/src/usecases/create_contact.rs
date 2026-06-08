use super::validation::{validate_name,
                        validate_optional_email};
use domain::{Contact,
             ContactRepository,
             DomainError};
use std::sync::Arc;

pub struct CreateContactUseCase {
    repository: Arc<dyn ContactRepository>,
}

impl CreateContactUseCase {
    pub fn new(repository: Arc<dyn ContactRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, name: String, email: Option<String>, phone: Option<String>, address: Option<String>) -> Result<Contact, DomainError> {
        validate_name(&name)?;
        validate_optional_email(email.as_ref())?;

        let contact = Contact::new(name, email, phone, address);
        self.repository.create(contact).await
    }
}
