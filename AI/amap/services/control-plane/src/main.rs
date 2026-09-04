//! Modernization Control Plane (design §1): policy, quality gate, risk, evidence, audit and
//! HITL over a REST API. Hosts the orchestrator in modular-monolith mode.
use amap_domain::*;
use amap_knowledge::{ReviewStatus, WorkflowRun};
use amap_orchestrator::AgentContext;
#[allow(unused_imports)]
use amap_platform::{Platform, PlatformBuilder, RunSpec, Settings};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone)]
struct App {
    platform: Arc<Platform>,
    api_token: Option<String>,
    spec_root: std::path::PathBuf,
    execution_root: std::path::PathBuf,
    allowed_executables: HashSet<String>,
    insecure_dev: bool,
}

#[derive(Clone)]
struct AuthActor(String);

type ApiResult<T> = Result<Json<T>, (StatusCode, String)>;

fn internal(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

fn constant_time_eq(expected: &str, supplied: &str) -> bool {
    use subtle::ConstantTimeEq;
    expected.as_bytes().ct_eq(supplied.as_bytes()).into()
}

async fn authenticate(
    State(app): State<App>,
    headers: HeaderMap,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some(expected) = &app.api_token {
        let supplied = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .unwrap_or_default();
        if !constant_time_eq(expected, supplied) {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    let actor = headers
        .get("x-amap-actor")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .unwrap_or("api-client")
        .to_string();
    request.extensions_mut().insert(AuthActor(actor));
    Ok(next.run(request).await)
}

#[derive(Deserialize)]
struct StartRun {
    /// Path to an `amap.toml` run spec on the control-plane host.
    spec: String,
    #[serde(default)]
    mock: bool,
}

fn launch_run(platform: Arc<Platform>, ctx: AgentContext) {
    let run_id = ctx.run_id.0.clone();
    tokio::spawn(async move {
        amap_telemetry::Metrics::global().inc("amap_runs_started_total", &[]);
        let result = amap_agents::run_modernization(ctx).await;
        if let Ok(Some(mut state)) = platform.knowledge.get_workflow_run(&run_id).await {
            match result {
                Ok(outcome) => {
                    state.status = if outcome.certified {
                        "certified".into()
                    } else if outcome.halted.is_some() {
                        "halted".into()
                    } else {
                        "failed".into()
                    };
                    let mut value = serde_json::to_value(&outcome).unwrap_or(Value::Null);
                    if let Some(gateway) = &platform.gateway {
                        value["llm_audit"] = json!(gateway.audit_log());
                    }
                    state.checkpoint =
                        serde_json::to_value(&outcome.final_outputs).unwrap_or(Value::Null);
                    state.outcome = Some(value);
                }
                Err(error) => {
                    state.status = "error".into();
                    state.outcome = Some(json!({ "error": error.to_string() }));
                }
            }
            state.updated_at = platform.clock.now();
            if let Err(error) = platform.knowledge.upsert_workflow_run(state).await {
                tracing::error!(%error, %run_id, "failed to persist workflow result");
            }
        }
    });
}

async fn start_run(State(app): State<App>, Json(req): Json<StartRun>) -> ApiResult<WorkflowRun> {
    if req.mock && !app.insecure_dev {
        return Err((
            StatusCode::FORBIDDEN,
            "request-scoped mock execution is disabled outside insecure development mode".into(),
        ));
    }
    let requested = std::path::Path::new(&req.spec);
    let requested = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        app.spec_root.join(requested)
    };
    let requested = std::fs::canonicalize(&requested)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid spec path: {e}")))?;
    if !requested.starts_with(&app.spec_root) {
        return Err((
            StatusCode::FORBIDDEN,
            "run specification is outside the configured spec root".into(),
        ));
    }
    let spec = RunSpec::load(&requested).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    if spec.run.auto_approve_hitl && !app.insecure_dev {
        return Err((
            StatusCode::FORBIDDEN,
            "auto_approve_hitl is disabled outside insecure development mode".into(),
        ));
    }
    spec.run
        .validate_execution_boundary(&app.execution_root, &app.allowed_executables)
        .map_err(|e| (StatusCode::FORBIDDEN, e))?;
    // Mock runs share the platform's stores but answer LLM calls from the spec's fixtures.
    let platform = if req.mock || app.platform.mock {
        Arc::new(
            app.platform
                .with_mock_fixtures(spec.mock.fixtures.as_deref()),
        )
    } else {
        app.platform.clone()
    };
    let ctx = platform.context_for(&spec, None).await.map_err(internal)?;
    let now = platform.clock.now();
    let state = WorkflowRun {
        id: ctx.run_id.0.clone(),
        function_id: ctx.function_id.clone(),
        status: "running".into(),
        spec_path: requested,
        mock: platform.mock,
        started_at: now,
        updated_at: now,
        outcome: None,
        checkpoint: json!({}),
    };
    app.platform
        .knowledge
        .upsert_workflow_run(state.clone())
        .await
        .map_err(internal)?;
    launch_run(platform, ctx);
    Ok(Json(state))
}

async fn list_runs(State(app): State<App>) -> ApiResult<Vec<WorkflowRun>> {
    app.platform
        .knowledge
        .list_workflow_runs()
        .await
        .map(Json)
        .map_err(internal)
}

async fn get_run(State(app): State<App>, Path(id): Path<String>) -> ApiResult<WorkflowRun> {
    app.platform
        .knowledge
        .get_workflow_run(&id)
        .await
        .map_err(internal)?
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "run not found".into()))
}

async fn resume_run(State(app): State<App>, Path(id): Path<String>) -> ApiResult<WorkflowRun> {
    let mut state = app
        .platform
        .knowledge
        .get_workflow_run(&id)
        .await
        .map_err(internal)?
        .ok_or((StatusCode::NOT_FOUND, "run not found".into()))?;
    if state.status != "halted" {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "only a halted run can be resumed, current status: {}",
                state.status
            ),
        ));
    }
    let reviews = app
        .platform
        .knowledge
        .list_reviews()
        .await
        .map_err(internal)?;
    if reviews.iter().any(|review| {
        review.run_id.as_ref().map(|run| run.as_str()) == Some(id.as_str())
            && review.status == ReviewStatus::Rejected
    }) {
        return Err((StatusCode::CONFLICT, "the HITL review was rejected".into()));
    }
    if !reviews.iter().any(|review| {
        review.run_id.as_ref().map(|run| run.as_str()) == Some(id.as_str())
            && review.status == ReviewStatus::Approved
    }) {
        return Err((
            StatusCode::CONFLICT,
            "an approved HITL review is required before resume".into(),
        ));
    }
    let spec = RunSpec::load(&state.spec_path)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    spec.run
        .validate_execution_boundary(&app.execution_root, &app.allowed_executables)
        .map_err(|error| (StatusCode::FORBIDDEN, error))?;
    let platform = if state.mock || app.platform.mock {
        Arc::new(
            app.platform
                .with_mock_fixtures(spec.mock.fixtures.as_deref()),
        )
    } else {
        app.platform.clone()
    };
    let mut ctx = platform
        .context_for(&spec, Some(state.id.clone()))
        .await
        .map_err(internal)?;
    ctx.inputs = state.checkpoint.clone();
    state.status = "running".into();
    state.updated_at = platform.clock.now();
    state.outcome = None;
    app.platform
        .knowledge
        .upsert_workflow_run(state.clone())
        .await
        .map_err(internal)?;
    launch_run(platform, ctx);
    Ok(Json(state))
}

async fn functions(State(app): State<App>) -> ApiResult<Vec<BusinessFunction>> {
    app.platform
        .knowledge
        .list_functions()
        .await
        .map(Json)
        .map_err(internal)
}

async fn function_detail(
    State(app): State<App>,
    Path((id, what)): Path<(String, String)>,
) -> ApiResult<Value> {
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
            let rows = app
                .platform
                .lake
                .summary(f.as_str())
                .await
                .map_err(internal)?;
            serde_json::to_value(rows)
        }
        _ => return Err((StatusCode::NOT_FOUND, "unknown resource".into())),
    };
    v.map(Json).map_err(internal)
}

async fn reviews(State(app): State<App>) -> ApiResult<Vec<amap_knowledge::ReviewRequest>> {
    app.platform
        .knowledge
        .list_reviews()
        .await
        .map(Json)
        .map_err(internal)
}

#[derive(Deserialize)]
struct Decide {
    status: ReviewStatus,
}

async fn decide(
    State(app): State<App>,
    axum::Extension(actor): axum::Extension<AuthActor>,
    Path(id): Path<String>,
    Json(d): Json<Decide>,
) -> ApiResult<Value> {
    if d.status == ReviewStatus::Pending {
        return Err((
            StatusCode::BAD_REQUEST,
            "a review decision must be approved or rejected".into(),
        ));
    }
    app.platform
        .knowledge
        .decide_review(&id, d.status, &actor.0)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
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

async fn evidence_query(
    State(app): State<App>,
    Query(q): Query<SqlQuery>,
) -> ApiResult<Vec<Value>> {
    if !q.sql.trim_start().to_uppercase().starts_with("SELECT") {
        return Err((
            StatusCode::BAD_REQUEST,
            "only SELECT queries are allowed".into(),
        ));
    }
    app.platform
        .lake
        .query(&q.sql)
        .await
        .map(Json)
        .map_err(internal)
}

async fn graph_dot(State(app): State<App>) -> Result<String, (StatusCode, String)> {
    let snap = app.platform.knowledge.snapshot().await.map_err(internal)?;
    Ok(amap_graph::KnowledgeGraph::from_snapshot(&snap).to_dot())
}

async fn audit(State(app): State<App>) -> Json<Value> {
    Json(
        app.platform
            .gateway
            .as_ref()
            .map(|g| json!(g.audit_log()))
            .unwrap_or(json!({ "note": "audit is served by the remote llm-gateway (/v1/audit)" })),
    )
}

async fn metrics() -> String {
    amap_telemetry::Metrics::global().render()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::load(None)?;
    if !settings.insecure_dev && settings.api_token.is_none() {
        anyhow::bail!(
            "AMAP_API_TOKEN is required; set AMAP_INSECURE_DEV=true only for isolated local development"
        );
    }
    amap_telemetry::init(&amap_telemetry::TelemetryConfig {
        service_name: "control-plane".into(),
        json: settings.json_logs,
        otlp_endpoint: settings.otlp_endpoint.clone(),
    });
    let mock = std::env::var("AMAP_MOCK_LLM")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);
    if mock && !settings.insecure_dev {
        anyhow::bail!("AMAP_MOCK_LLM requires AMAP_INSECURE_DEV=true");
    }
    let mut builder = PlatformBuilder::new(settings.clone());
    if mock {
        builder = builder.with_mock_fixtures(None);
    }
    let platform = Arc::new(builder.build().await?);
    let app = App {
        platform,
        api_token: settings.api_token.clone(),
        spec_root: std::fs::canonicalize(&settings.spec_root)?,
        execution_root: std::fs::canonicalize(&settings.worker_root)?,
        allowed_executables: settings
            .worker_allowed_executables
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect(),
        insecure_dev: settings.insecure_dev,
    };
    let protected = Router::new()
        .route("/metrics", get(metrics))
        .route("/v1/runs", post(start_run).get(list_runs))
        .route("/v1/runs/{id}", get(get_run))
        .route("/v1/runs/{id}/resume", post(resume_run))
        .route("/v1/functions", get(functions))
        .route("/v1/functions/{id}/{what}", get(function_detail))
        .route("/v1/reviews", get(reviews))
        .route("/v1/reviews/{id}/decide", post(decide))
        .route("/v1/events", get(events))
        .route("/v1/evidence/query", get(evidence_query))
        .route("/v1/graph.dot", get(graph_dot))
        .route("/v1/llm/audit", get(audit))
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .route_layer(middleware::from_fn_with_state(app.clone(), authenticate));
    let router = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .merge(protected)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(app);
    let listener = tokio::net::TcpListener::bind(&settings.control_plane_listen).await?;
    tracing::info!(addr = %settings.control_plane_listen, "control-plane listening");
    axum::serve(listener, router).await?;
    Ok(())
}
