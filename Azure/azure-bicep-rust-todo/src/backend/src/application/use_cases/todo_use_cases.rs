use crate::application::use_cases::TodoRepository;
use crate::domain::entities::{CreateTodoRequest, Todo, UpdateTodoRequest};
use std::sync::Arc;

pub struct TodoUseCases {
    repository: Arc<dyn TodoRepository + Send + Sync>,
}

impl TodoUseCases {
    pub fn new(repository: Arc<dyn TodoRepository + Send + Sync>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn get_all_todos(&self) -> Result<Vec<Todo>, Box<dyn std::error::Error>> { self.repository.get_all().await }

    pub async fn get_todo_by_id(&self, id: &str) -> Result<Option<Todo>, Box<dyn std::error::Error>> { self.repository.get_by_id(id).await }

    pub async fn create_todo(&self, request: CreateTodoRequest) -> Result<Todo, Box<dyn std::error::Error>> {
        let todo = Todo::new(request.title, request.description);
        self.repository.create(&todo).await
    }

    pub async fn update_todo(&self, id: &str, request: UpdateTodoRequest) -> Result<Option<Todo>, Box<dyn std::error::Error>> {
        if let Some(mut todo) = self.repository.get_by_id(id).await? {
            todo.update(request);
            Ok(Some(self.repository.update(&todo).await?))
        } else {
            Ok(None)
        }
    }

    pub async fn delete_todo(&self, id: &str) -> Result<bool, Box<dyn std::error::Error>> { self.repository.delete(id).await }
}
