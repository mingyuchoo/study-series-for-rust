//! Modernization Control Plane (design §1): policy, quality gate, risk, evidence, audit and
//! HITL over a REST API. Hosts the orchestrator in modular-monolith mode.
use amap_cli::{Platform, PlatformBuilder, RunSpec, Settings};
#[allow(unused_imports)]
use amap_cli::bootstrap::mock_provider;
use amap_domain::*;
use amap_knowledge::ReviewStatus;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Serialize)]
struct RunState {
    run_id: String,
    function_id: String,
    status: String,
    started_at: chrono::DateTime<chrono::Utc>,
    outcome: Option<Value>,
}

#[derive(Clone)]
struct App {
    platform: Arc<Platform>,
    runs: Arc<Mutex<HashMap<String, RunState>>>,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, String)>;

fn internal(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

#[derive(Deserialize)]
struct StartRun {
    /// Path to an `amap.toml` run spec on the control-plane host.
    spec: String,
    #[serde(default)]
    mock: bool,
}

async fn start_run(State(app): State<App>, Json(req): Json<StartRun>) -> ApiResult<RunState> {
    let spec = RunSpec::load(std::path::Path::new(&req.spec)).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    // Mock runs share the platform's stores but answer LLM calls from the spec's fixtures.
    let platform = if req.mock || app.platform.mock { Arc::new(app.platform.with_mock_fixtures(spec.mock.fixtures.as_deref())) } else { app.platform.clone() };
    let ctx = platform.context_for(&spec, None).await.map_err(internal)?;
    let state = RunState { run_id: ctx.run_id.0.clone(), function_id: ctx.function_id.0.clone(), status: "running".into(), started_at: chrono::Utc::now(), outcome: None };
    app.runs.lock().unwrap().insert(state.run_id.clone(), state.clone());
    let runs = app.runs.clone();
    let run_id = state.run_id.clone();
    let run_platform = platform.clone();
    tokio::spawn(async move {
        amap_telemetry::Metrics::global().inc("amap_runs_started_total", &[]);
        let result = amap_agents::run_modernization(ctx).await;
        let mut r = runs.lock().unwrap();
        if let Some(s) = r.get_mut(&run_id) {
            match result {
                Ok(o) => {
                    s.status = if o.certified { "certified".into() } else if o.halted.is_some() { "halted".into() } else { "failed".into() };
                    let mut v = serde_json::to_value(&o).unwrap_or(Value::Null);
                    if let Some(gw) = &run_platform.gateway {
                        v["llm_audit"] = json!(gw.audit_log());
                    }
                    s.outcome = Some(v);
                }
                Err(e) => {
                    s.status = "error".into();
                    s.outcome = Some(json!({ "error": e.to_string() }));
                }
            }
        }
    });
    Ok(Json(state))
}

async fn list_runs(State(app): State<App>) -> Json<Vec<RunState>> {
    Json(app.runs.lock().unwrap().values().cloned().collect())
}

async fn get_run(State(app): State<App>, Path(id): Path<String>) -> ApiResult<RunState> {
    app.runs.lock().unwrap().get(&id).cloned().map(Json).ok_or((StatusCode::NOT_FOUND, "run not found".into()))
}

async fn functions(State(app): State<App>) -> ApiResult<Vec<BusinessFunction>> {
    app.platform.knowledge.list_functions().await.map(Json).map_err(internal)
}

async fn function_detail(State(app): State<App>, Path((id, what)): Path<(String, String)>) -> ApiResult<Value> {
    let k = &app.platform.knowledge;
    let f = FunctionId::new(id);
    let v = match what.as_str() {
        "rules" => serde_json::to_value(k.rules_for(&f).await.map_err(internal)?),
        "behaviors" => serde_json::to_value(k.behaviors_for(&f).await.map_err(internal)?),
        "scenarios" => serde_json::to_value(k.scenarios_for(&f).await.map_err(internal)?),
        "decisions" => serde_json::to_value(k.decisions_for(&f).await.map_err(internal)?),
        "evidence" => serde_json::to_value(k.evidence_for(&f).await.map_err(internal)?),
        "requirements" => serde_json::to_value(k.requirements_for(&f).await.map_err(internal)?),
        "certificate" => {
            let rows = app.platform.lake.summary(f.as_str()).await.map_err(internal)?;
            serde_json::to_value(rows)
        }
        _ => return Err((StatusCode::NOT_FOUND, "unknown resource".into())),
    };
    v.map(Json).map_err(internal)
}

async fn reviews(State(app): State<App>) -> ApiResult<Vec<amap_knowledge::ReviewRequest>> {
    app.platform.knowledge.list_reviews().await.map(Json).map_err(internal)
}

#[derive(Deserialize)]
struct Decide {
    status: ReviewStatus,
    by: String,
}

async fn decide(State(app): State<App>, Path(id): Path<String>, Json(d): Json<Decide>) -> ApiResult<Value> {
    app.platform.knowledge.decide_review(&id, d.status, &d.by).await.map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    amap_telemetry::Metrics::global().inc("amap_hitl_decisions_total", &[]);
    Ok(Json(json!({ "ok": true })))
}

async fn events(State(app): State<App>) -> Json<Vec<amap_orchestrator::Event>> {
    Json(app.platform.bus.history())
}

#[derive(Deserialize)]
struct SqlQuery {
    sql: String,
}

async fn evidence_query(State(app): State<App>, Query(q): Query<SqlQuery>) -> ApiResult<Vec<Value>> {
    if !q.sql.trim_start().to_uppercase().starts_with("SELECT") {
        return Err((StatusCode::BAD_REQUEST, "only SELECT queries are allowed".into()));
    }
    app.platform.lake.query(&q.sql).await.map(Json).map_err(internal)
}

async fn graph_dot(State(app): State<App>) -> Result<String, (StatusCode, String)> {
    let snap = app.platform.knowledge.snapshot().await.map_err(internal)?;
    Ok(amap_graph::KnowledgeGraph::from_snapshot(&snap).to_dot())
}

async fn audit(State(app): State<App>) -> Json<Value> {
    Json(app.platform.gateway.as_ref().map(|g| json!(g.audit_log())).unwrap_or(json!({ "note": "audit is served by the remote llm-gateway (/v1/audit)" })))
}

async fn metrics() -> String {
    amap_telemetry::Metrics::global().render()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::load(None)?;
    amap_telemetry::init(&amap_telemetry::TelemetryConfig { service_name: "control-plane".into(), json: settings.json_logs, otlp_endpoint: settings.otlp_endpoint.clone() });
    let mock = std::env::var("AMAP_MOCK_LLM").map(|v| v == "1" || v == "true").unwrap_or(false);
    let mut builder = PlatformBuilder::new(settings.clone());
    if mock {
        builder = builder.with_mock_fixtures(None);
    }
    let platform = Arc::new(builder.build().await?);
    let app = App { platform, runs: Arc::new(Mutex::new(HashMap::new())) };
    let router = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/metrics", get(metrics))
        .route("/v1/runs", post(start_run).get(list_runs))
        .route("/v1/runs/{id}", get(get_run))
        .route("/v1/functions", get(functions))
        .route("/v1/functions/{id}/{what}", get(function_detail))
        .route("/v1/reviews", get(reviews))
        .route("/v1/reviews/{id}/decide", post(decide))
        .route("/v1/events", get(events))
        .route("/v1/evidence/query", get(evidence_query))
        .route("/v1/graph.dot", get(graph_dot))
        .route("/v1/llm/audit", get(audit))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(app);
    let listener = tokio::net::TcpListener::bind(&settings.control_plane_listen).await?;
    tracing::info!(addr = %settings.control_plane_listen, "control-plane listening");
    axum::serve(listener, router).await?;
    Ok(())
}
