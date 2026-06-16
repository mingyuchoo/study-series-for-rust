// presentation/web.rs - Web server for User CRUD UI

use axum::{Json,
           Router,
           extract::{Path,
                     State},
           http::StatusCode,
           response::{Html,
                      IntoResponse,
                      Response},
           routing::get};
use infrastructure::controllers::UserDbController;
use serde::{Deserialize,
            Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

// Shared controller type
pub type SharedController = Arc<Mutex<UserDbController<infrastructure::repositories::UserDbRepository>>>;

#[derive(Serialize, Deserialize)]
pub struct UserInput {
    pub username: String,
    pub email: String,
}

async fn list_users(State(controller): State<SharedController>) -> Response {
    let ctrl = controller.lock().await;
    match ctrl.list_all_users_json() {
        | Ok(users) => axum::Json::<Vec<application::services::UserDto>>(users).into_response(),
        | Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_user(Path(id): Path<String>, State(controller): State<SharedController>) -> Response {
    let ctrl = controller.lock().await;
    match ctrl.get_user_details_json(&id) {
        | Some(user_dto) => axum::Json::<application::services::UserDto>(user_dto).into_response(),
        | None => (StatusCode::NOT_FOUND, "User not found").into_response(),
    }
}

async fn create_user(State(controller): State<SharedController>, Json(user): Json<UserInput>) -> Response {
    let ctrl = controller.lock().await;
    match ctrl.register_user(user.username, user.email) {
        | Ok(msg) => Html(msg.to_string()).into_response(),
        | Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn update_user(Path(id): Path<String>, State(controller): State<SharedController>) -> Response {
    let ctrl = controller.lock().await;
    match ctrl.deactivate_user(&id) {
        | Ok(msg) => Html(msg.to_string()).into_response(),
        | Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn delete_user(Path(id): Path<String>, State(controller): State<SharedController>) -> Response {
    let ctrl = controller.lock().await;
    match ctrl.delete_user(&id) {
        | Ok(msg) => Html(msg.to_string()).into_response(),
        | Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn index() -> Html<&'static str> { Html(include_str!("./static/index.html")) }

pub async fn run_server() {
    let db_path = "users.db";
    let controller = UserDbController::new_with_db_path(db_path).expect("Failed to init controller");
    let shared = Arc::new(Mutex::new(controller));

    // Create the router with the shared state
    let app = Router::new()
        .route("/", get(index))
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/:id", get(get_user).put(update_user).delete(delete_user))
        .with_state(shared);

    println!("Web UI running at http://localhost:3000");

    // Start the server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
