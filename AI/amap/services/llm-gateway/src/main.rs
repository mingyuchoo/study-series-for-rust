//! Standalone LLM Gateway (stack §13): agents call `POST /v1/complete`; this service owns
//! provider routing, PII filtering, caching, retries, budgets, cost and audit.
use amap_cli::Settings;
use amap_domain::ModelProvider;
use amap_llm::{
    AnthropicProvider, Gateway, GatewayConfig, LlmClient, LlmError, LlmRequest, OpenAiProvider,
    Router, RouterConfig,
};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router as AxumRouter};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    gateway: Arc<Gateway>,
    token: Option<String>,
}

fn constant_time_eq(expected: &str, supplied: &str) -> bool {
    use subtle::ConstantTimeEq;
    expected.as_bytes().ct_eq(supplied.as_bytes()).into()
}

async fn authenticate(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some(expected) = &state.token {
        let supplied = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .unwrap_or_default();
        if !constant_time_eq(expected, supplied) {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    Ok(next.run(request).await)
}

async fn complete(
    State(s): State<AppState>,
    Json(req): Json<LlmRequest>,
) -> Result<Json<amap_llm::LlmResponse>, (StatusCode, String)> {
    amap_telemetry::Metrics::global()
        .inc("amap_llm_requests_total", &[("role", req.role.as_str())]);
    match s.gateway.complete(req).await {
        Ok(r) => Ok(Json(r)),
        Err(LlmError::Transient(m)) => Err((StatusCode::SERVICE_UNAVAILABLE, m)),
        Err(LlmError::BudgetExceeded(r)) => Err((
            StatusCode::TOO_MANY_REQUESTS,
            format!("budget exceeded for {r}"),
        )),
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
    if !settings.insecure_dev && settings.llm_gateway_token.is_none() {
        anyhow::bail!(
            "AMAP_LLM_GATEWAY_TOKEN is required; set AMAP_INSECURE_DEV=true only for isolated local development"
        );
    }
    amap_telemetry::init(&amap_telemetry::TelemetryConfig {
        service_name: "llm-gateway".into(),
        json: settings.json_logs,
        otlp_endpoint: settings.otlp_endpoint.clone(),
    });
    let mut router = Router::new(RouterConfig::default());
    if let Some(p) = AnthropicProvider::from_env() {
        router = router.with_provider(ModelProvider::Anthropic, Arc::new(p));
    }
    if let Some(p) = OpenAiProvider::from_env() {
        router = router.with_provider(ModelProvider::OpenAi, Arc::new(p));
    }
    if let (Ok(url), Ok(model)) = (
        std::env::var("AMAP_LOCAL_LLM_URL"),
        std::env::var("AMAP_LOCAL_LLM_MODEL"),
    ) {
        router = router.with_provider(
            ModelProvider::Local,
            Arc::new(
                OpenAiProvider::new(
                    std::env::var("AMAP_LOCAL_LLM_KEY").unwrap_or_default(),
                    model,
                )
                .with_base_url(url)
                .local(),
            ),
        );
    }
    if router.configured().is_empty() {
        if !settings.insecure_dev {
            anyhow::bail!(
                "no LLM provider configured: set ANTHROPIC_API_KEY or OPENAI_API_KEY with AMAP_OPENAI_MODEL"
            );
        }
        tracing::warn!("no providers configured; serving a development-only mock provider");
        router = router.with_provider(ModelProvider::Mock, Arc::new(amap_llm::MockProvider::new()));
    }
    tracing::info!(providers = ?router.configured(), "gateway providers");
    let gateway = Arc::new(Gateway::new(
        router,
        GatewayConfig {
            run_token_budget: settings.token_budget,
            ..Default::default()
        },
    ));
    let state = AppState {
        gateway,
        token: settings.llm_gateway_token.clone(),
    };
    let protected = AxumRouter::new()
        .route("/metrics", get(metrics))
        .route("/v1/complete", post(complete))
        .route("/v1/audit", get(audit))
        .layer(DefaultBodyLimit::max(4 * 1024 * 1024))
        .route_layer(middleware::from_fn_with_state(state.clone(), authenticate));
    let app = AxumRouter::new()
        .route("/healthz", get(|| async { "ok" }))
        .merge(protected)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(&settings.llm_gateway_listen).await?;
    tracing::info!(addr = %settings.llm_gateway_listen, "llm-gateway listening");
    axum::serve(listener, app).await?;
    Ok(())
}
