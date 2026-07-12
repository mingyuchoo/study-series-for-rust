use crate::{adapters::{http::app_state::AppState,
                       {self}},
            infra::{openapi::ApiDoc,
                    setup::init_tracing}};
use axum::{Router,
           http};
use http::header::{AUTHORIZATION,
                   CONTENT_TYPE};
use tower_http::{cors::CorsLayer,
                 trace::TraceLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;

pub fn create_app(app_state: AppState) -> Router {
    init_tracing();

    let cors = CorsLayer::new()
        .allow_origin("http://localhost:5173".parse::<http::HeaderValue>().unwrap())
        .allow_methods([http::Method::POST, http::Method::GET, http::Method::PUT, http::Method::DELETE])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION])
        .allow_credentials(true);

    let api_router = Router::new()
        .nest("/api", adapters::http::routes::router())
        .with_state(app_state)
        .layer(cors)
        .layer(TraceLayer::new_for_http().make_span_with(|request: &http::Request<_>| {
            let request_id = Uuid::new_v4();
            tracing::info_span!(
                "http-request",
                method = %request.method(),
                uri = %request.uri(),
                version = ?request.version(),
                request_id = %request_id
            )
        }));

    // Mount Swagger UI and OpenAPI JSON
    let swagger = SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi());

    api_router.merge(swagger)
}
