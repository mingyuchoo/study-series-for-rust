use crate::domain::{PersonData,
                    PersonRepository};
use anyhow::Result;
use std::sync::Arc;

pub struct PersonUseCases {
    repository: Arc<dyn PersonRepository + Send + Sync>,
}

impl PersonUseCases {
    pub fn new(repository: Arc<dyn PersonRepository + Send + Sync>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn create_person(&self, name: String) -> Result<String> {
        let person_data = PersonData {
            name,
            id: None,
        };
        match self.repository.create_person(person_data).await? {
            | Some(person) => Ok(format!("{:?}", person)),
            | None => Ok("[]".to_string()),
        }
    }

    pub async fn delete_person(&self, id: String) -> Result<String> {
        let id_option = if id.is_empty() { None } else { Some(id) };
        self.repository.delete_person(id_option).await
    }

    pub async fn list_people(&self) -> Result<String> {
        let people = self.repository.list_people().await?;
        Ok(format!("{:?}", people))
    }
}
