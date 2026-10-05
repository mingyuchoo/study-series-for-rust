use actix_web::{App,
                http::StatusCode,
                test,
                web};
use backend::{adapters::http::{create_todo,
                               delete_todo,
                               get_todos,
                               update_todo},
              domain::entities::Todo,
              infrastructure::{db::initialize_database,
                               openapi::ApiDoc,
                               setup::AppState}};
use serde_json::{Value,
                 json};
use sqlx::sqlite::SqlitePoolOptions;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[actix_web::test]
async fn sqlite_crud_and_openapi_remain_compatible() {
    let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    initialize_database(&pool).await.unwrap();
    let state = AppState::new(pool);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.todo_use_cases))
            .service(SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", ApiDoc::openapi()))
            .service(
                web::scope("/api")
                    .service(get_todos)
                    .service(create_todo)
                    .service(update_todo)
                    .service(delete_todo),
            ),
    )
    .await;

    let request = test::TestRequest::post()
        .uri("/api/todos")
        .set_json(json!({"title": "Dependency upgrade", "description": "SQLite round trip"}))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let created: Todo = test::read_body_json(response).await;
    assert!(!created.completed);
    assert_eq!(created.description.as_deref(), Some("SQLite round trip"));
    uuid::Uuid::parse_str(&created.id).unwrap();

    let request = test::TestRequest::get().uri("/api/todos").to_request();
    let todos: Vec<Todo> = test::call_and_read_body_json(&app, request).await;
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].id, created.id);
    assert_eq!(todos[0].created_at, created.created_at);

    let uri = format!("/api/todos/{}", created.id);
    let request = test::TestRequest::put().uri(&uri).set_json(json!({"completed": true})).to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let updated: Todo = test::read_body_json(response).await;
    assert!(updated.completed);
    assert_eq!(updated.title, created.title);

    let request = test::TestRequest::get().uri("/api-docs/openapi.json").to_request();
    let document: Value = test::call_and_read_body_json(&app, request).await;
    assert_eq!(document["openapi"], "3.1.0");
    assert!(document["components"]["schemas"]["Todo"].is_object());
    assert!(document["paths"]["/api/todos"]["post"].is_object());
    assert!(document["paths"]["/api/todos/{id}"]["put"].is_object());
    let request = test::TestRequest::get().uri("/swagger-ui/").to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = test::read_body(response).await;
    assert!(std::str::from_utf8(&html).unwrap().contains("swagger-ui"));

    let request = test::TestRequest::delete().uri(&uri).to_request();
    assert_eq!(test::call_service(&app, request).await.status(), StatusCode::NO_CONTENT);
    let request = test::TestRequest::delete().uri(&uri).to_request();
    assert_eq!(test::call_service(&app, request).await.status(), StatusCode::NOT_FOUND);
    let request = test::TestRequest::get().uri("/api/todos").to_request();
    let todos: Vec<Todo> = test::call_and_read_body_json(&app, request).await;
    assert!(todos.is_empty());
}
