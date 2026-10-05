use crate::adapters::persistence::SqliteTodoRepository;
use crate::application::use_cases::{TodoRepository, TodoUseCases};
use sqlx::SqlitePool;
use std::sync::Arc;

pub struct AppState {
    pub todo_use_cases: Arc<TodoUseCases>,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        let todo_repository: Arc<dyn TodoRepository + Send + Sync> = Arc::new(SqliteTodoRepository::new(pool));
        let todo_use_cases = Arc::new(TodoUseCases::new(todo_repository));

        Self {
            todo_use_cases,
        }
    }
}
