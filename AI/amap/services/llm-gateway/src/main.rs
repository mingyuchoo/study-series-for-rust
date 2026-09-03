//! Standalone LLM Gateway (stack §13): agents call `POST /v1/complete`; this service owns
//! provider routing, PII filtering, caching, retries, budgets, cost and audit.
use amap_cli::Settings;
use amap_domain::ModelProvider;
use amap_llm::{AnthropicProvider, Gateway, GatewayConfig, LlmClient, LlmError, LlmRequest, OpenAiProvider, Router, RouterConfig};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router as AxumRouter};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    gateway: Arc<Gateway>,
}

async fn complete(State(s): State<AppState>, Json(req): Json<LlmRequest>) -> Result<Json<amap_llm::LlmResponse>, (StatusCode, String)> {
    amap_telemetry::Metrics::global().inc("amap_llm_requests_total", &[("role", req.role.as_str())]);
    match s.gateway.complete(req).await {
        Ok(r) => Ok(Json(r)),
        Err(LlmError::Transient(m)) => Err((StatusCode::SERVICE_UNAVAILABLE, m)),
        Err(LlmError::BudgetExceeded(r)) => Err((StatusCode::TOO_MANY_REQUESTS, format!("budget exceeded for {r}"))),
        Err(LlmError::Refused(m)) => Err((StatusCode::UNPROCESSABLE_ENTITY, m)),
        Err(e) => Err((StatusCode::BAD_GATEWAY, e.to_string())),
    }
}

async fn audit(State(s): State<AppState>) -> Json<Vec<amap_llm::AuditEntry>> {
    Json(s.gateway.audit_log())
}

async fn metrics() -> String {
    amap_telemetry::Metrics::global().render()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::load(None)?;
    amap_telemetry::init(&amap_telemetry::TelemetryConfig { service_name: "llm-gateway".into(), json: settings.json_logs, otlp_endpoint: settings.otlp_endpoint.clone() });
    let mut router = Router::new(RouterConfig::default());
    if let Some(p) = AnthropicProvider::from_env() {
        router = router.with_provider(ModelProvider::Anthropic, Arc::new(p));
    }
    if let Some(p) = OpenAiProvider::from_env() {
        router = router.with_provider(ModelProvider::OpenAi, Arc::new(p));
    }
    if let (Ok(url), Ok(model)) = (std::env::var("AMAP_LOCAL_LLM_URL"), std::env::var("AMAP_LOCAL_LLM_MODEL")) {
        router = router.with_provider(ModelProvider::Local, Arc::new(OpenAiProvider::new(std::env::var("AMAP_LOCAL_LLM_KEY").unwrap_or_default(), model).with_base_url(url).local()));
    }
    if router.configured().is_empty() {
        tracing::warn!("no providers configured; serving a mock provider (set ANTHROPIC_API_KEY / OPENAI_API_KEY)");
        router = router.with_provider(ModelProvider::Mock, Arc::new(amap_llm::MockProvider::new()));
    }
    tracing::info!(providers = ?router.configured(), "gateway providers");
    let gateway = Arc::new(Gateway::new(router, GatewayConfig { run_token_budget: settings.token_budget, ..Default::default() }));
    let app = AxumRouter::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/metrics", get(metrics))
        .route("/v1/complete", post(complete))
        .route("/v1/audit", get(audit))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(AppState { gateway });
    let listener = tokio::net::TcpListener::bind(&settings.llm_gateway_listen).await?;
    tracing::info!(addr = %settings.llm_gateway_listen, "llm-gateway listening");
    axum::serve(listener, app).await?;
    Ok(())
}
