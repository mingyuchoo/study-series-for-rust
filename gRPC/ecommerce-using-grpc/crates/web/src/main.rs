use anyhow::{Context,
             Result};
use axum::extract::{Path,
                    State};
use axum::http::StatusCode;
use axum::response::{Html,
                     IntoResponse,
                     Response};
use axum::routing::get;
use axum::{Json,
           Router};
use proto::product_info_client::ProductInfoClient;
use proto::{Empty,
            Product,
            ProductId};
use serde::{Deserialize,
            Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tonic_health::pb::HealthCheckRequest;
use tonic_health::pb::health_check_response::ServingStatus;
use tonic_health::pb::health_client::HealthClient;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{Level,
              info};
use tracing_subscriber::FmtSubscriber;

const INDEX_HTML: &str = include_str!("index.html");

#[derive(Clone)]
struct AppState {
    grpc_url: String,
    start_time: Instant,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    connected: bool,
    grpc_address: String,
    latency_ms: Option<f64>,
    uptime_seconds: u64,
}

#[derive(Serialize, Deserialize)]
struct ProductJson {
    id: i32,
    name: String,
    description: String,
    price: f32,
}

impl From<Product> for ProductJson {
    fn from(p: Product) -> Self {
        Self {
            id: p.id,
            name: p.name,
            description: p.description,
            price: p.price,
        }
    }
}

#[derive(Deserialize)]
struct CreateProductRequest {
    name: String,
    description: Option<String>,
    price: f32,
}

#[derive(Serialize)]
struct CreateProductResponse {
    id: i32,
    message: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn error_json(status: StatusCode, msg: impl Into<String>) -> Response {
    (
        status,
        Json(ErrorResponse {
            error: msg.into(),
        }),
    )
        .into_response()
}

async fn index_handler() -> Html<&'static str> { Html(INDEX_HTML) }

async fn health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let uptime_seconds = state.start_time.elapsed().as_secs();
    let ping_start = Instant::now();

    let channel = match tonic::transport::Channel::from_shared(state.grpc_url.clone()) {
        | Ok(endpoint) => match endpoint.connect().await {
            | Ok(c) => c,
            | Err(_) => {
                return Json(HealthResponse {
                    status: "DISCONNECTED".to_string(),
                    connected: false,
                    grpc_address: state.grpc_url.clone(),
                    latency_ms: None,
                    uptime_seconds,
                });
            },
        },
        | Err(_) => {
            return Json(HealthResponse {
                status: "INVALID_URL".to_string(),
                connected: false,
                grpc_address: state.grpc_url.clone(),
                latency_ms: None,
                uptime_seconds,
            });
        },
    };

    let mut client = HealthClient::new(channel);
    let request = tonic::Request::new(HealthCheckRequest {
        service: "".to_string(),
    });

    match client.check(request).await {
        | Ok(response) => {
            let latency_ms = ping_start.elapsed().as_secs_f64() * 1000.0;
            let serving_status = response.into_inner().status();

            let status_str = match serving_status {
                | ServingStatus::Serving => "SERVING",
                | ServingStatus::NotServing => "NOT_SERVING",
                | ServingStatus::ServiceUnknown => "SERVICE_UNKNOWN",
                | ServingStatus::Unknown => "UNKNOWN",
            };

            Json(HealthResponse {
                status: status_str.to_string(),
                connected: serving_status == ServingStatus::Serving,
                grpc_address: state.grpc_url.clone(),
                latency_ms: Some(latency_ms),
                uptime_seconds,
            })
        },
        | Err(_) => Json(HealthResponse {
            status: "NOT_SERVING".to_string(),
            connected: false,
            grpc_address: state.grpc_url.clone(),
            latency_ms: None,
            uptime_seconds,
        }),
    }
}

async fn list_products_handler(State(state): State<Arc<AppState>>) -> Response {
    let mut client = match ProductInfoClient::connect(state.grpc_url.clone()).await {
        | Ok(c) => c,
        | Err(e) => return error_json(StatusCode::SERVICE_UNAVAILABLE, format!("gRPC 서버 연결 실패: {}", e)),
    };

    match client.list_products(tonic::Request::new(Empty {})).await {
        | Ok(resp) => {
            let products: Vec<ProductJson> = resp.into_inner().products.into_iter().map(Into::into).collect();
            Json(products).into_response()
        },
        | Err(e) => error_json(StatusCode::INTERNAL_SERVER_ERROR, format!("상품 목록 조회 오류: {}", e.message())),
    }
}

async fn get_product_handler(State(state): State<Arc<AppState>>, Path(id): Path<i32>) -> Response {
    let mut client = match ProductInfoClient::connect(state.grpc_url.clone()).await {
        | Ok(c) => c,
        | Err(e) => return error_json(StatusCode::SERVICE_UNAVAILABLE, format!("gRPC 서버 연결 실패: {}", e)),
    };

    match client
        .get_product(tonic::Request::new(ProductId {
            id,
        }))
        .await
    {
        | Ok(resp) => {
            let p: ProductJson = resp.into_inner().into();
            Json(p).into_response()
        },
        | Err(e) =>
            if e.code() == tonic::Code::NotFound {
                error_json(StatusCode::NOT_FOUND, format!("상품 ID {}를 찾을 수 없습니다.", id))
            } else {
                error_json(StatusCode::INTERNAL_SERVER_ERROR, format!("상품 조회 오류: {}", e.message()))
            },
    }
}

async fn add_product_handler(State(state): State<Arc<AppState>>, Json(payload): Json<CreateProductRequest>) -> Response {
    if payload.name.trim().is_empty() {
        return error_json(StatusCode::BAD_REQUEST, "상품명은 비워둘 수 없습니다.");
    }
    if payload.price <= 0.0 {
        return error_json(StatusCode::BAD_REQUEST, "상품 가격은 0보다 커야 합니다.");
    }

    let mut client = match ProductInfoClient::connect(state.grpc_url.clone()).await {
        | Ok(c) => c,
        | Err(e) => return error_json(StatusCode::SERVICE_UNAVAILABLE, format!("gRPC 서버 연결 실패: {}", e)),
    };

    let new_product = Product {
        id: 0,
        name: payload.name.trim().to_string(),
        description: payload.description.unwrap_or_default().trim().to_string(),
        price: payload.price,
    };

    match client.add_product(tonic::Request::new(new_product)).await {
        | Ok(resp) => {
            let assigned_id = resp.into_inner().id;
            (
                StatusCode::CREATED,
                Json(CreateProductResponse {
                    id: assigned_id,
                    message: "상품이 성공적으로 등록되었습니다.".to_string(),
                }),
            )
                .into_response()
        },
        | Err(e) => error_json(StatusCode::INTERNAL_SERVER_ERROR, format!("상품 등록 오류: {}", e.message())),
    }
}

fn init_tracing() -> Result<()> {
    let subscriber = FmtSubscriber::builder().with_max_level(Level::INFO).finish();
    tracing::subscriber::set_global_default(subscriber).context("Failed to set tracing subscriber")?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing()?;

    let grpc_url = std::env::var("GRPC_SERVER_URL").unwrap_or_else(|_| "http://[::1]:50051".to_string());
    let web_port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let web_addr: SocketAddr = format!("0.0.0.0:{}", web_port).parse().context("Failed to parse web address")?;

    let state = Arc::new(AppState {
        grpc_url: grpc_url.clone(),
        start_time: Instant::now(),
    });

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/health", get(health_handler))
        .route("/api/products", get(list_products_handler).post(add_product_handler))
        .route("/api/products/{id}", get(get_product_handler))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    info!("🚀 Web 대시보드가 시작되었습니다: http://localhost:{}", web_port);
    info!("🔗 연결된 gRPC 백엔드 주소: {}", grpc_url);

    let listener = TcpListener::bind(web_addr).await.context("Failed to bind TCP listener")?;

    axum::serve(listener, app).await.context("Web server failed")?;

    Ok(())
}
