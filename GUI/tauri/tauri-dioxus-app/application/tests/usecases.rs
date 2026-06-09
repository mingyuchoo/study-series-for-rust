//! Use case unit tests driven by an in-memory mock `ContactRepository`.
//!
//! These exercise the orchestration and validation logic in the application
//! layer without touching SQLite, so they stay fast and deterministic.

use application::{CreateContactUseCase,
                  DeleteContactUseCase,
                  GetContactUseCase,
                  ListContactsUseCase,
                  SearchContactsUseCase,
                  UpdateContactUseCase};
use async_trait::async_trait;
use domain::{Contact,
             ContactRepository,
             DomainError};
use std::{collections::HashMap,
          sync::{Arc,
                 Mutex}};
use uuid::Uuid;

#[derive(Default)]
struct MockContactRepository {
    contacts: Mutex<HashMap<Uuid, Contact>>,
}

impl MockContactRepository {
    fn arc() -> Arc<Self> { Arc::new(Self::default()) }

    fn count(&self) -> usize { self.contacts.lock().unwrap().len() }
}

#[async_trait]
impl ContactRepository for MockContactRepository {
    async fn create(&self, contact: Contact) -> Result<Contact, DomainError> {
        self.contacts.lock().unwrap().insert(contact.id, contact.clone());
        Ok(contact)
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<Contact>, DomainError> { Ok(self.contacts.lock().unwrap().get(&id).cloned()) }

    async fn get_all(&self) -> Result<Vec<Contact>, DomainError> {
        let mut contacts: Vec<Contact> = self.contacts.lock().unwrap().values().cloned().collect();
        contacts.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(contacts)
    }

    async fn update(&self, contact: Contact) -> Result<Contact, DomainError> {
        self.contacts.lock().unwrap().insert(contact.id, contact.clone());
        Ok(contact)
    }

    async fn delete(&self, id: Uuid) -> Result<(), DomainError> {
        self.contacts.lock().unwrap().remove(&id);
        Ok(())
    }

    async fn search(&self, query: &str) -> Result<Vec<Contact>, DomainError> {
        let query = query.to_lowercase();
        let matches = |value: &Option<String>| value.as_ref().is_some_and(|v| v.to_lowercase().contains(&query));
        let contacts = self
            .contacts
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.name.to_lowercase().contains(&query) || matches(&c.email) || matches(&c.phone) || matches(&c.address))
            .cloned()
            .collect();
        Ok(contacts)
    }
}

#[tokio::test]
async fn create_rejects_blank_name() {
    let repository = MockContactRepository::arc();
    let use_case = CreateContactUseCase::new(repository.clone());

    let result = use_case.execute("   ".to_string(), None, None, None).await;

    assert!(matches!(result, Err(DomainError::InvalidContactData(_))));
    assert_eq!(repository.count(), 0);
}

#[tokio::test]
async fn create_rejects_invalid_email() {
    let repository = MockContactRepository::arc();
    let use_case = CreateContactUseCase::new(repository.clone());

    let result = use_case
        .execute("Ada".to_string(), Some("not-an-email".to_string()), None, None)
        .await;

    assert!(matches!(result, Err(DomainError::InvalidContactData(_))));
    assert_eq!(repository.count(), 0);
}

#[tokio::test]
async fn create_rejects_non_korean_mobile_phone() {
    let repository = MockContactRepository::arc();
    let use_case = CreateContactUseCase::new(repository.clone());

    let result = use_case
        .execute("Ada".to_string(), None, Some("choo".to_string()), None)
        .await;

    assert!(matches!(result, Err(DomainError::InvalidContactData(_))));
    assert_eq!(repository.count(), 0);
}

#[tokio::test]
async fn create_persists_valid_contact() {
    let repository = MockContactRepository::arc();
    let use_case = CreateContactUseCase::new(repository.clone());

    let contact = use_case
        .execute("Ada".to_string(), Some("ada@example.com".to_string()), None, None)
        .await
        .expect("contact should be created");

    assert_eq!(contact.name, "Ada");
    assert_eq!(repository.count(), 1);
}

#[tokio::test]
async fn get_returns_not_found_for_missing_contact() {
    let repository = MockContactRepository::arc();
    let use_case = GetContactUseCase::new(repository);

    let result = use_case.execute(Uuid::new_v4()).await;

    assert!(matches!(result, Err(DomainError::ContactNotFound)));
}

#[tokio::test]
async fn update_returns_not_found_for_missing_contact() {
    let repository = MockContactRepository::arc();
    let use_case = UpdateContactUseCase::new(repository);

    let result = use_case
        .execute(Uuid::new_v4(), Some("Grace".to_string()), None, None, None)
        .await;

    assert!(matches!(result, Err(DomainError::ContactNotFound)));
}

#[tokio::test]
async fn update_rejects_invalid_email_before_lookup() {
    let repository = MockContactRepository::arc();
    let use_case = UpdateContactUseCase::new(repository);

    let result = use_case
        .execute(Uuid::new_v4(), None, Some("bad".to_string()), None, None)
        .await;

    // Validation must fail with a validation error, not a not-found error.
    assert!(matches!(result, Err(DomainError::InvalidContactData(_))));
}

#[tokio::test]
async fn update_mutates_existing_contact() {
    let repository = MockContactRepository::arc();
    let created = CreateContactUseCase::new(repository.clone())
        .execute("Ada".to_string(), None, None, None)
        .await
        .expect("contact should be created");

    let updated = UpdateContactUseCase::new(repository.clone())
        .execute(created.id, Some("Ada Lovelace".to_string()), Some("ada@example.com".to_string()), None, None)
        .await
        .expect("contact should be updated");

    assert_eq!(updated.name, "Ada Lovelace");
    assert_eq!(updated.email, Some("ada@example.com".to_string()));
    assert!(updated.updated_at >= created.updated_at);
}

#[tokio::test]
async fn delete_removes_contact() {
    let repository = MockContactRepository::arc();
    let created = CreateContactUseCase::new(repository.clone())
        .execute("Ada".to_string(), None, None, None)
        .await
        .expect("contact should be created");

    DeleteContactUseCase::new(repository.clone())
        .execute(created.id)
        .await
        .expect("contact should be deleted");

    assert_eq!(repository.count(), 0);
}

#[tokio::test]
async fn list_and_search_return_expected_contacts() {
    let repository = MockContactRepository::arc();
    let create = CreateContactUseCase::new(repository.clone());
    create
        .execute("Ada".to_string(), Some("ada@example.com".to_string()), None, None)
        .await
        .unwrap();
    create
        .execute("Grace".to_string(), None, Some("010-1234-5678".to_string()), None)
        .await
        .unwrap();

    let all = ListContactsUseCase::new(repository.clone()).execute().await.expect("list should succeed");
    assert_eq!(all.len(), 2);

    let matches = SearchContactsUseCase::new(repository)
        .execute("grace")
        .await
        .expect("search should succeed");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "Grace");
}
