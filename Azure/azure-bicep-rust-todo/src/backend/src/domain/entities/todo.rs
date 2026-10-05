use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "title": "Learn Rust",
    "description": "Study Rust programming language",
    "completed": false,
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
}))]
pub struct Todo {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: String,
    #[schema(example = "Learn Rust")]
    pub title: String,
    #[schema(example = "Study Rust programming language")]
    pub description: Option<String>,
    #[schema(example = false)]
    pub completed: bool,
    #[schema(example = "2023-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,
    #[schema(example = "2023-01-01T00:00:00Z")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize, Debug, ToSchema)]
#[schema(example = json!({
    "title": "Learn Rust",
    "description": "Study Rust programming language"
}))]
pub struct CreateTodoRequest {
    #[schema(example = "Learn Rust")]
    pub title: String,
    #[schema(example = "Study Rust programming language")]
    pub description: Option<String>,
}

#[derive(Deserialize, Debug, ToSchema)]
#[schema(example = json!({
    "title": "Learn Advanced Rust",
    "description": "Study advanced Rust concepts",
    "completed": true
}))]
pub struct UpdateTodoRequest {
    #[schema(example = "Learn Advanced Rust")]
    pub title: Option<String>,
    #[schema(example = "Study advanced Rust concepts")]
    pub description: Option<String>,
    #[schema(example = true)]
    pub completed: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[schema(example = json!({"error": "Todo not found"}))]
pub struct ErrorResponse {
    #[schema(example = "Todo not found")]
    pub error: String,
}

impl Todo {
    pub fn new(title: String, description: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            description,
            completed: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn update(&mut self, request: UpdateTodoRequest) {
        if let Some(title) = request.title {
            self.title = title;
        }
        if let Some(description) = request.description {
            self.description = Some(description);
        }
        if let Some(completed) = request.completed {
            self.completed = completed;
        }
        self.updated_at = Utc::now();
    }
}
