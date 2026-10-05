use crate::adapters::http::{create_todo, delete_todo, get_todos, update_todo};
use crate::infrastructure::openapi::ApiDoc;
use crate::infrastructure::setup::AppState;
use actix_files as fs;
use actix_web::middleware::Logger;
use actix_web::{App, HttpServer, web};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub async fn create_app(app_state: AppState, port: u16) -> std::io::Result<()> {
    println!("Starting TODO server on port {}", port);
    println!("Swagger UI available at: http://localhost:{}/swagger-ui/", port);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.todo_use_cases.clone()))
            .wrap(Logger::default())
            // Swagger UI
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", ApiDoc::openapi())
            )
            // API routes
            .service(
                web::scope("/api")
                    .service(get_todos)
                    .service(create_todo)
                    .service(update_todo)
                    .service(delete_todo)
            )
            // Static files (frontend)
            .service(fs::Files::new("/", "./wwwroot").index_file("index.html"))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
