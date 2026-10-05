use crate::domain::entities::Todo;
use async_trait::async_trait;

#[async_trait]
pub trait TodoRepository {
    async fn get_all(&self) -> Result<Vec<Todo>, Box<dyn std::error::Error>>;
    async fn get_by_id(&self, id: &str) -> Result<Option<Todo>, Box<dyn std::error::Error>>;
    async fn create(&self, todo: &Todo) -> Result<Todo, Box<dyn std::error::Error>>;
    async fn update(&self, todo: &Todo) -> Result<Todo, Box<dyn std::error::Error>>;
    async fn delete(&self, id: &str) -> Result<bool, Box<dyn std::error::Error>>;
}
