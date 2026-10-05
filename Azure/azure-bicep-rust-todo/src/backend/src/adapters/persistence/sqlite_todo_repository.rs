use crate::application::use_cases::TodoRepository;
use crate::domain::entities::Todo;
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Row, SqlitePool};

pub struct SqliteTodoRepository {
    pool: SqlitePool,
}

impl SqliteTodoRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
        }
    }
}

#[async_trait]
impl TodoRepository for SqliteTodoRepository {
    async fn get_all(&self) -> Result<Vec<Todo>, Box<dyn std::error::Error>> {
        let rows = sqlx::query("SELECT * FROM todos ORDER BY created_at DESC").fetch_all(&self.pool).await?;

        let todos: Vec<Todo> = rows
            .iter()
            .map(|row| Todo {
                id: row.get("id"),
                title: row.get("title"),
                description: row.get("description"),
                completed: row.get("completed"),
                created_at: row.get::<String, _>("created_at").parse().unwrap_or_else(|_| Utc::now()),
                updated_at: row.get::<String, _>("updated_at").parse().unwrap_or_else(|_| Utc::now()),
            })
            .collect();

        Ok(todos)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Todo>, Box<dyn std::error::Error>> {
        let row = sqlx::query("SELECT * FROM todos WHERE id = ?").bind(id).fetch_optional(&self.pool).await?;

        if let Some(row) = row {
            Ok(Some(Todo {
                id: row.get("id"),
                title: row.get("title"),
                description: row.get("description"),
                completed: row.get("completed"),
                created_at: row.get::<String, _>("created_at").parse().unwrap_or_else(|_| Utc::now()),
                updated_at: row.get::<String, _>("updated_at").parse().unwrap_or_else(|_| Utc::now()),
            }))
        } else {
            Ok(None)
        }
    }

    async fn create(&self, todo: &Todo) -> Result<Todo, Box<dyn std::error::Error>> {
        sqlx::query("INSERT INTO todos (id, title, description, completed, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&todo.id)
            .bind(&todo.title)
            .bind(&todo.description)
            .bind(todo.completed)
            .bind(todo.created_at.to_rfc3339())
            .bind(todo.updated_at.to_rfc3339())
            .execute(&self.pool)
            .await?;

        Ok(todo.clone())
    }

    async fn update(&self, todo: &Todo) -> Result<Todo, Box<dyn std::error::Error>> {
        sqlx::query("UPDATE todos SET title = ?, description = ?, completed = ?, updated_at = ? WHERE id = ?")
            .bind(&todo.title)
            .bind(&todo.description)
            .bind(todo.completed)
            .bind(todo.updated_at.to_rfc3339())
            .bind(&todo.id)
            .execute(&self.pool)
            .await?;

        Ok(todo.clone())
    }

    async fn delete(&self, id: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let result = sqlx::query("DELETE FROM todos WHERE id = ?").bind(id).execute(&self.pool).await?;

        Ok(result.rows_affected() > 0)
    }
}
