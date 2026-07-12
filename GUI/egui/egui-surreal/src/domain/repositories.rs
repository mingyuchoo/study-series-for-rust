use super::entities::{AuthParams,
                      Person,
                      PersonData};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait PersonRepository {
    async fn create_person(&self, person_data: PersonData) -> Result<Option<Person>>;
    async fn delete_person(&self, id: Option<String>) -> Result<String>;
    async fn list_people(&self) -> Result<Vec<Person>>;
}

#[async_trait]
pub trait AuthRepository {
    async fn sign_up(&self) -> Result<String>;
    async fn sign_in(&self, params: AuthParams) -> Result<String>;
    async fn sign_in_root(&self) -> Result<String>;
    async fn get_session(&self) -> Result<String>;
}

#[async_trait]
pub trait QueryRepository {
    async fn execute_raw_query(&self, query: String) -> Result<String>;
}
