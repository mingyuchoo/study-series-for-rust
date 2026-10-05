use crate::domain::entities::{CreateTodoRequest, ErrorResponse, Todo, UpdateTodoRequest};
use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityScheme};
use utoipa::{Modify, OpenApi};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::adapters::http::get_todos,
        crate::adapters::http::create_todo,
        crate::adapters::http::update_todo,
        crate::adapters::http::delete_todo,
    ),
    components(
        schemas(Todo, CreateTodoRequest, UpdateTodoRequest, ErrorResponse)
    ),
    tags(
        (name = "todos", description = "Todo management endpoints.")
    ),
    info(
        title = "Clean Architecture TODO API",
        description = "A RESTful API for managing todos built with Clean Architecture principles in Rust",
        version = "1.0.0",
        contact(
            name = "API Support",
            email = "support@todoapi.com",
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT",
        ),
    ),
    servers(
        (url = "http://localhost:8080", description = "Local development server"),
        (url = "http://localhost:8000", description = "Alternative local server"),
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme("api_key", SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)))
        }
    }
}
