use crate::application::use_cases::TodoUseCases;
use crate::domain::entities::{CreateTodoRequest, ErrorResponse, Todo, UpdateTodoRequest};
use actix_web::{HttpResponse, Responder, Result, delete, get, post, put, web};
use serde_json::json;
use std::sync::Arc;
use utoipa;

#[utoipa::path(
    get,
    path = "/api/todos",
    responses(
        (status = 200, description = "List all todos successfully", body = [Todo]),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "todos"
)]
#[get("/todos")]
pub async fn get_todos(use_cases: web::Data<Arc<TodoUseCases>>) -> Result<impl Responder> {
    match use_cases.get_all_todos().await {
        | Ok(todos) => Ok(HttpResponse::Ok().json(todos)),
        | Err(e) => {
            eprintln!("Error getting todos: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "error": "Failed to get todos"
            })))
        },
    }
}

#[utoipa::path(
    post,
    path = "/api/todos",
    request_body = CreateTodoRequest,
    responses(
        (status = 201, description = "Todo created successfully", body = Todo),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "todos"
)]
#[post("/todos")]
pub async fn create_todo(use_cases: web::Data<Arc<TodoUseCases>>, request: web::Json<CreateTodoRequest>) -> Result<impl Responder> {
    match use_cases.create_todo(request.into_inner()).await {
        | Ok(todo) => Ok(HttpResponse::Created().json(todo)),
        | Err(e) => {
            eprintln!("Error creating todo: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "error": "Failed to create todo"
            })))
        },
    }
}

#[utoipa::path(
    put,
    path = "/api/todos/{id}",
    request_body = UpdateTodoRequest,
    responses(
        (status = 200, description = "Todo updated successfully", body = Todo),
        (status = 404, description = "Todo not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    params(
        ("id" = String, Path, description = "Todo database id")
    ),
    tag = "todos"
)]
#[put("/todos/{id}")]
pub async fn update_todo(use_cases: web::Data<Arc<TodoUseCases>>, path: web::Path<String>, request: web::Json<UpdateTodoRequest>) -> Result<impl Responder> {
    let id = path.into_inner();

    match use_cases.update_todo(&id, request.into_inner()).await {
        | Ok(Some(todo)) => Ok(HttpResponse::Ok().json(todo)),
        | Ok(None) => Ok(HttpResponse::NotFound().json(json!({
            "error": "Todo not found"
        }))),
        | Err(e) => {
            eprintln!("Error updating todo: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "error": "Failed to update todo"
            })))
        },
    }
}

#[utoipa::path(
    delete,
    path = "/api/todos/{id}",
    responses(
        (status = 204, description = "Todo deleted successfully"),
        (status = 404, description = "Todo not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    params(
        ("id" = String, Path, description = "Todo database id")
    ),
    tag = "todos"
)]
#[delete("/todos/{id}")]
pub async fn delete_todo(use_cases: web::Data<Arc<TodoUseCases>>, path: web::Path<String>) -> Result<impl Responder> {
    let id = path.into_inner();

    match use_cases.delete_todo(&id).await {
        | Ok(true) => Ok(HttpResponse::NoContent().finish()),
        | Ok(false) => Ok(HttpResponse::NotFound().json(json!({
            "error": "Todo not found"
        }))),
        | Err(e) => {
            eprintln!("Error deleting todo: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "error": "Failed to delete todo"
            })))
        },
    }
}
