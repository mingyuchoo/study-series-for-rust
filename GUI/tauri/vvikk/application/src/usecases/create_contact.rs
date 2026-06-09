use super::validation::{validate_name,
                        validate_optional_email,
                        validate_optional_phone};
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

    pub async fn execute(&self, name: String, email: Option<String>, phone: Option<String>, memo: Option<String>) -> Result<Contact, DomainError> {
        validate_name(&name)?;
        validate_optional_email(email.as_ref())?;
        validate_optional_phone(phone.as_ref())?;

        let contact = Contact::new(name, email, phone, memo);
        self.repository.create(contact).await
    }
}
