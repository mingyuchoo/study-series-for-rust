use crate::{adapters::http::app_state::AppState,
            app_error::AppResult,
            use_cases::user::{UserListItem,
                              UserUseCases}};
use axum::{Json,
           Router,
           extract::{Path,
                     State},
           http::StatusCode,
           response::IntoResponse,
           routing::{delete,
                     get,
                     post,
                     put}};
use secrecy::SecretString;
use serde::{Deserialize,
            Serialize};
use std::sync::Arc;
use tracing::{info,
              instrument};
use utoipa::ToSchema;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(post_user))
        .route("/", get(list_users))
        .route("/{id}", get(get_user))
        .route("/{id}", put(update_user))
        .route("/{id}", delete(delete_user))
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub(crate) struct RegisterPayload {
    username: String,
    email: String,
    #[schema(value_type = String)]
    password: SecretString,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct RegisterResponse {
    success: bool,
}

/// Creates a new user based on the submitted credentials.
#[instrument(skip(user_use_cases))]
#[utoipa::path(
    post,
    path = "/api/user",
    request_body = RegisterPayload,
    responses(
        (status = 201, description = "User created", body = RegisterResponse)
    ),
    tag = "user"
)]
pub(crate) async fn post_user(State(user_use_cases): State<Arc<UserUseCases>>, Json(payload): Json<RegisterPayload>) -> AppResult<impl IntoResponse> {
    info!("Register user called");
    user_use_cases.add(&payload.username, &payload.email, &payload.password).await?;

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            success: true,
        }),
    ))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct UserListItemResponse {
    id: uuid::Uuid,
    username: String,
    email: String,
    created_at: chrono::NaiveDateTime,
}

#[instrument(skip(user_use_cases))]
#[utoipa::path(
    get,
    path = "/api/user",
    responses(
        (status = 200, description = "Users listed", body = [UserListItemResponse])
    ),
    tag = "user"
)]
pub(crate) async fn list_users(State(user_use_cases): State<Arc<UserUseCases>>) -> AppResult<impl IntoResponse> {
    info!("List users called");
    let items: Vec<UserListItem> = user_use_cases.list().await?;
    let resp: Vec<UserListItemResponse> = items
        .into_iter()
        .map(|u| UserListItemResponse {
            id: u.id,
            username: u.username,
            email: u.email,
            created_at: u.created_at,
        })
        .collect();

    Ok((StatusCode::OK, Json(resp)))
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub(crate) struct UpdatePayload {
    username: Option<String>,
    email: Option<String>,
    #[schema(value_type = Option<String>)]
    password: Option<SecretString>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct UpdateResponse {
    success: bool,
}

#[instrument(skip(user_use_cases))]
#[utoipa::path(
    put,
    path = "/api/user/{id}",
    request_body = UpdatePayload,
    params(("id" = uuid::Uuid, Path, description = "User id")),
    responses(
        (status = 200, description = "User updated", body = UpdateResponse)
    ),
    tag = "user"
)]
pub(crate) async fn update_user(
    State(user_use_cases): State<Arc<UserUseCases>>,
    Path(id): Path<uuid::Uuid>,
    Json(payload): Json<UpdatePayload>,
) -> AppResult<impl IntoResponse> {
    info!("Update user called");
    user_use_cases
        .update(id, payload.username.as_deref(), payload.email.as_deref(), payload.password.as_ref())
        .await?;

    Ok((
        StatusCode::OK,
        Json(UpdateResponse {
            success: true,
        }),
    ))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct DeleteResponse {
    success: bool,
}

#[instrument(skip(user_use_cases))]
#[utoipa::path(
    delete,
    path = "/api/user/{id}",
    params(("id" = uuid::Uuid, Path, description = "User id")),
    responses(
        (status = 204, description = "User deleted", body = DeleteResponse)
    ),
    tag = "user"
)]
pub(crate) async fn delete_user(State(user_use_cases): State<Arc<UserUseCases>>, Path(id): Path<uuid::Uuid>) -> AppResult<impl IntoResponse> {
    info!("Delete user called");
    user_use_cases.delete(id).await?;
    Ok((
        StatusCode::NO_CONTENT,
        Json(DeleteResponse {
            success: true,
        }),
    ))
}

#[instrument(skip(user_use_cases))]
#[utoipa::path(
    get,
    path = "/api/user/{id}",
    params(("id" = uuid::Uuid, Path, description = "User id")),
    responses(
        (status = 200, description = "User found", body = UserListItemResponse),
        (status = 404, description = "User not found")
    ),
    tag = "user"
)]
pub(crate) async fn get_user(State(user_use_cases): State<Arc<UserUseCases>>, Path(id): Path<uuid::Uuid>) -> AppResult<impl IntoResponse> {
    info!("Get user called");
    let maybe = user_use_cases.get(id).await?;
    let resp = match maybe {
        | Some(u) => (
            StatusCode::OK,
            Json(UserListItemResponse {
                id: u.id,
                username: u.username,
                email: u.email,
                created_at: u.created_at,
            }),
        )
            .into_response(),
        | None => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "success": false }))).into_response(),
    };
    Ok(resp)
}
