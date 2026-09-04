//! Modernization Control Plane (design §1): policy, quality gate, risk, evidence, audit and
//! HITL over a REST API. Hosts the orchestrator in modular-monolith mode.
mod auth;

use amap_domain::*;
use amap_knowledge::{ReviewStatus, WorkflowRun};
use amap_orchestrator::AgentContext;
#[allow(unused_imports)]
use amap_platform::{Platform, PlatformBuilder, RunSpec, Settings};
use amap_policy::{ActionContext, Principal};
use auth::{OidcConfig, OidcVerifier};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use futures::{stream, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::convert::Infallible;
use std::path::Path as FilePath;
use std::sync::Arc;
use std::time::Duration;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
struct App {
    platform: Arc<Platform>,
    /// Shared secret for service clients (CI, automation). Never sufficient for HITL decisions.
    api_token: Option<String>,
    /// Verifies human bearer tokens issued by the configured identity provider.
    oidc: Option<Arc<OidcVerifier>>,
    /// Role a human must hold to decide HITL reviews; unset accepts any verified subject.
    reviewer_role: Option<String>,
    spec_root: std::path::PathBuf,
    execution_root: std::path::PathBuf,
    allowed_executables: HashSet<String>,
    insecure_dev: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthMethod {
    /// Identity verified against the OIDC issuer (signature, issuer, audience, expiry).
    Oidc,
    /// Shared service token; the actor name is not verified.
    ServiceToken,
    /// No authentication configured (isolated development only).
    InsecureDev,
}

/// The authenticated caller attached to every protected request.
#[derive(Clone, Debug)]
struct AuthActor {
    id: String,
    method: AuthMethod,
    roles: Vec<String>,
    issuer: Option<String>,
}

impl AuthActor {
    fn via(&self) -> String {
        match self.method {
            AuthMethod::Oidc => format!("oidc:{}", self.issuer.as_deref().unwrap_or_default()),
            AuthMethod::ServiceToken => "service-token".into(),
            AuthMethod::InsecureDev => "insecure-dev-header".into(),
        }
    }
}

fn header_actor(headers: &HeaderMap) -> String {
    headers
        .get("x-amap-actor")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .unwrap_or("api-client")
        .to_string()
}

type ApiResult<T> = Result<Json<T>, (StatusCode, String)>;

fn internal(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

fn constant_time_eq(expected: &str, supplied: &str) -> bool {
    use subtle::ConstantTimeEq;
    expected.as_bytes().ct_eq(supplied.as_bytes()).into()
}

/// Resolve the caller. Order: shared service token, then an OIDC bearer token, then (only when
/// nothing is configured, i.e. insecure development) the self-declared `X-AMAP-Actor` header.
async fn resolve_actor(app: &App, headers: &HeaderMap) -> Result<AuthActor, StatusCode> {
    let supplied = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .unwrap_or_default();
    if let Some(expected) = &app.api_token {
        if !supplied.is_empty() && constant_time_eq(expected, supplied) {
            return Ok(AuthActor {
                // The header is only trusted where nothing can be trusted anyway.
                id: if app.insecure_dev {
                    header_actor(headers)
                } else {
                    "api-client".into()
                },
                method: AuthMethod::ServiceToken,
                roles: vec![],
                issuer: None,
            });
        }
    }
    if let Some(oidc) = &app.oidc {
        if supplied.is_empty() {
            return Err(StatusCode::UNAUTHORIZED);
        }
        return match oidc.verify(supplied).await {
            Ok(identity) => Ok(AuthActor {
                id: identity.subject,
                method: AuthMethod::Oidc,
                roles: identity.roles,
                issuer: Some(identity.issuer),
            }),
            Err(error) => {
                tracing::warn!(%error, "rejected bearer token");
                Err(StatusCode::UNAUTHORIZED)
            }
        };
    }
    if app.api_token.is_none() {
        return Ok(AuthActor {
            id: header_actor(headers),
            method: AuthMethod::InsecureDev,
            roles: vec![],
            issuer: None,
        });
    }
    Err(StatusCode::UNAUTHORIZED)
}

async fn authenticate(
    State(app): State<App>,
    headers: HeaderMap,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let actor = resolve_actor(&app, &headers).await?;
    request.extensions_mut().insert(actor);
    Ok(next.run(request).await)
}

#[derive(Deserialize)]
struct StartRun {
    /// Path to an `amap.toml` run spec on the control-plane host.
    spec: String,
    #[serde(default)]
    mock: bool,
}

#[derive(Clone, Debug, Serialize)]
struct SpecSummary {
    path: String,
    function_id: String,
    name: String,
    domain: String,
    priority: Priority,
    description: String,
    mock_available: bool,
}

fn discover_run_specs(root: &FilePath) -> Vec<SpecSummary> {
    let mut pending = vec![root.to_path_buf()];
    let mut specs = Vec::new();

    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !name.starts_with('.') && name != "target" && name != "node_modules" {
                    pending.push(path);
                }
                continue;
            }
            if path.extension().and_then(|value| value.to_str()) != Some("toml") {
                continue;
            }
            let Ok(spec) = RunSpec::load(&path) else {
                continue;
            };
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            specs.push(SpecSummary {
                path: relative.to_string_lossy().replace('\\', "/"),
                function_id: spec.function.id,
                name: spec.function.name,
                domain: spec.function.domain,
                priority: spec.function.priority,
                description: spec.function.description,
                mock_available: spec.mock.fixtures.is_some(),
            });
        }
    }
    specs.sort_by(|left, right| left.path.cmp(&right.path));
    specs
}

async fn list_specs(State(app): State<App>) -> Json<Vec<SpecSummary>> {
    Json(discover_run_specs(&app.spec_root))
}

async fn openapi() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/yaml; charset=utf-8")],
        include_str!("../../../openapi/amap.yaml"),
    )
}

fn to_sse_event(event: amap_orchestrator::Event) -> SseEvent {
    let id = format!(
        "{}:{}",
        event.at.timestamp_micros(),
        event.subject.replace(' ', "-")
    );
    let data = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
    SseEvent::default().id(id).event(event.subject).data(data)
}

async fn run_events(
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, (StatusCode, String)> {
    if app
        .platform
        .knowledge
        .get_workflow_run(&id)
        .await
        .map_err(internal)?
        .is_none()
    {
        return Err((StatusCode::NOT_FOUND, "run not found".into()));
    }

    // Subscribe before taking the history snapshot so no event can be lost during hand-off.
    // A client may see a duplicate at this boundary and should de-duplicate by the SSE id.
    let receiver = app.platform.bus.subscribe();
    let history_run_id = id.clone();
    let history = app
        .platform
        .bus
        .history()
        .into_iter()
        .filter(move |event| event.run_id == history_run_id)
        .map(|event| Ok(to_sse_event(event)));
    let live = stream::unfold((receiver, id), |(mut receiver, id)| async move {
        loop {
            match receiver.recv().await {
                Ok(event) if event.run_id == id => {
                    return Some((Ok(to_sse_event(event)), (receiver, id)));
                }
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    Ok(Sse::new(stream::iter(history).chain(live)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
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
                    match platform.knowledge.llm_audit(Some(&run_id), 10_000).await {
                        Ok(entries) => value["llm_audit"] = json!(entries),
                        Err(error) => {
                            tracing::warn!(%error, %run_id, "could not attach the LLM audit ledger to the outcome")
                        }
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

/// Record a HITL decision. The decider must be a human verified by the identity provider
/// (outside insecure development), and Cedar decides whether that human may act.
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
    if actor.method != AuthMethod::Oidc && !app.insecure_dev {
        return Err((
            StatusCode::FORBIDDEN,
            "HITL decisions require a reviewer authenticated by the configured OIDC issuer; \
             the service token cannot approve or reject reviews"
                .into(),
        ));
    }
    let review = app
        .platform
        .knowledge
        .list_reviews()
        .await
        .map_err(internal)?
        .into_iter()
        .find(|review| review.id == id)
        .ok_or((StatusCode::NOT_FOUND, format!("not found: {id}")))?;
    let has_required_role = app
        .reviewer_role
        .as_ref()
        .map(|role| actor.roles.iter().any(|held| held == role))
        .unwrap_or(true);
    let decision = app
        .platform
        .policy
        .authorize(
            &Principal::human(&actor.id),
            "decide_review",
            &id,
            &ActionContext {
                is_critical: review.tier == HitlTier::SmeMandatory,
                role_required: app.reviewer_role.is_some(),
                has_required_role,
                ..ActionContext::default().with_uncertainty(review.uncertainty)
            },
        )
        .map_err(internal)?;
    if !decision.allowed {
        return Err((
            StatusCode::FORBIDDEN,
            format!(
                "policy denied decide_review for {}: {}",
                actor.id,
                decision.reasons.join(", ")
            ),
        ));
    }
    app.platform
        .knowledge
        .decide_review(&id, d.status, &actor.id, &actor.via())
        .await
        .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;
    amap_telemetry::Metrics::global().inc(
        "amap_hitl_decisions_total",
        &[
            ("via", &actor.via()),
            ("status", &format!("{:?}", d.status)),
        ],
    );
    tracing::info!(review = %id, actor = %actor.id, via = %actor.via(), status = ?d.status, "HITL decision recorded");
    Ok(Json(
        json!({ "ok": true, "decided_by": actor.id, "decided_via": actor.via() }),
    ))
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

#[derive(Deserialize)]
struct AuditQuery {
    run_id: Option<String>,
    limit: Option<usize>,
}

/// Durable LLM audit ledger (newest first). Shared with a remote llm-gateway when both use the
/// same database; without a database the ledger is process-local.
async fn audit(
    State(app): State<App>,
    Query(q): Query<AuditQuery>,
) -> ApiResult<Vec<LlmAuditEntry>> {
    app.platform
        .knowledge
        .llm_audit(q.run_id.as_deref(), q.limit.unwrap_or(500).clamp(1, 10_000))
        .await
        .map(Json)
        .map_err(internal)
}

async fn metrics() -> String {
    amap_telemetry::Metrics::global().render()
}

fn oidc_config(settings: &Settings) -> Option<OidcConfig> {
    settings.oidc_issuer.as_ref().map(|issuer| OidcConfig {
        issuer: issuer.trim_end_matches('/').to_string(),
        audience: settings.oidc_audience.clone(),
        jwks_url: settings.oidc_jwks_url.clone(),
        roles_claim: settings.oidc_roles_claim.clone(),
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load provider credentials (AZURE_OPENAI_*, ANTHROPIC_API_KEY, ...) from ./.env when present.
    let _ = dotenvy::dotenv();
    let settings = Settings::load(None)?;
    let oidc_settings = oidc_config(&settings);
    if !settings.insecure_dev && settings.api_token.is_none() && oidc_settings.is_none() {
        anyhow::bail!(
            "AMAP_API_TOKEN or AMAP_OIDC_ISSUER is required; set AMAP_INSECURE_DEV=true only for isolated local development"
        );
    }
    amap_telemetry::init(&amap_telemetry::TelemetryConfig {
        service_name: "control-plane".into(),
        json: settings.json_logs,
        otlp_endpoint: settings.otlp_endpoint.clone(),
    });
    let oidc = match oidc_settings {
        Some(config) => {
            if config.audience.is_none() {
                tracing::warn!("AMAP_OIDC_AUDIENCE is unset; tokens for other clients of the same issuer will be accepted");
            }
            let verifier = OidcVerifier::discover(config.clone())
                .await
                .map_err(|e| anyhow::anyhow!("OIDC setup failed: {e}"))?;
            tracing::info!(issuer = %config.issuer, roles_claim = %config.roles_claim, reviewer_role = ?settings.oidc_reviewer_role, "OIDC reviewer authentication enabled");
            Some(Arc::new(verifier))
        }
        None => {
            if !settings.insecure_dev {
                tracing::warn!("no OIDC issuer configured; HITL decisions will be rejected until AMAP_OIDC_ISSUER is set");
            }
            None
        }
    };
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
        oidc,
        reviewer_role: settings.oidc_reviewer_role.clone(),
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
    let router = build_router(app, &settings.web_dist);
    let listener = tokio::net::TcpListener::bind(&settings.control_plane_listen).await?;
    tracing::info!(addr = %settings.control_plane_listen, "control-plane listening");
    axum::serve(listener, router).await?;
    Ok(())
}

fn build_router(app: App, web_dist: &FilePath) -> Router {
    let protected = Router::new()
        .route("/metrics", get(metrics))
        .route("/v1/specs", get(list_specs))
        .route("/v1/runs", post(start_run).get(list_runs))
        .route("/v1/runs/{id}", get(get_run))
        .route("/v1/runs/{id}/events", get(run_events))
        .route("/v1/runs/{id}/resume", post(resume_run))
        .route("/v1/functions", get(functions))
        .route("/v1/functions/{id}/{what}", get(function_detail))
        .route("/v1/reviews", get(reviews))
        .route("/v1/reviews/{id}/decide", post(decide))
        .route("/v1/events", get(events))
        .route("/v1/evidence/query", get(evidence_query))
        .route("/v1/graph.dot", get(graph_dot))
        .route("/v1/llm/audit", get(audit))
        .route(
            "/v1/{*path}",
            any(|| async { (StatusCode::NOT_FOUND, "API route not found") }),
        )
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .route_layer(middleware::from_fn_with_state(app.clone(), authenticate));
    let router = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/openapi.yaml", get(openapi))
        .merge(protected)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(app);
    let web_index = web_dist.join("index.html");
    if web_index.is_file() {
        router.fallback_service(ServeDir::new(web_dist).fallback(ServeFile::new(web_index)))
    } else {
        router
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_run_spec_is_discoverable() {
        let root = FilePath::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(FilePath::parent)
            .unwrap();
        let specs = discover_run_specs(root);
        assert!(specs
            .iter()
            .any(|spec| spec.path == "examples/loan-demo/amap.toml"));
    }

    use amap_knowledge::ReviewRequest;
    use auth::testing::{self, token};
    use serde_json::json;

    async fn test_app(reviewer_role: Option<&str>) -> (App, Arc<Platform>) {
        let scratch = std::env::temp_dir().join(format!("amap-cp-{}", uuid::Uuid::new_v4()));
        let settings = Settings {
            lake: scratch.join("lake").display().to_string(),
            ..Settings::default()
        };
        let platform = Arc::new(
            PlatformBuilder::new(settings)
                .with_mock_fixtures(None)
                .build()
                .await
                .unwrap(),
        );
        platform
            .knowledge
            .queue_review(ReviewRequest {
                id: "HITL-1".into(),
                run_id: Some(RunId::new("RUN-1")),
                function_id: FunctionId::new("FN-1"),
                tier: HitlTier::SmeMandatory,
                reason: "test".into(),
                uncertainty: 0.3,
                status: ReviewStatus::Pending,
                requested_at: chrono::Utc::now(),
                decided_by: None,
                decided_via: None,
                decided_at: None,
            })
            .await
            .unwrap();
        let app = App {
            platform: platform.clone(),
            api_token: Some("service-secret".into()),
            oidc: Some(Arc::new(OidcVerifier::with_static_keys(
                testing::config(),
                testing::jwks(),
            ))),
            reviewer_role: reviewer_role.map(str::to_owned),
            spec_root: std::env::temp_dir(),
            execution_root: std::env::temp_dir(),
            allowed_executables: HashSet::new(),
            insecure_dev: false,
        };
        (app, platform)
    }

    async fn serve(app: App) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router = build_router(app, FilePath::new("/nonexistent"));
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        format!("http://{addr}")
    }

    async fn decide_with(base: &str, bearer: &str, extra: &[(&str, &str)]) -> (u16, String) {
        let client = reqwest::Client::new();
        let mut request = client
            .post(format!("{base}/v1/reviews/HITL-1/decide"))
            .bearer_auth(bearer)
            .json(&json!({ "status": "approved" }));
        for (name, value) in extra {
            request = request.header(*name, *value);
        }
        let response = request.send().await.unwrap();
        (response.status().as_u16(), response.text().await.unwrap())
    }

    #[tokio::test]
    async fn hitl_decision_requires_verified_human_with_role() {
        let (app, platform) = test_app(Some("sme")).await;
        let base = serve(app).await;

        // The shared service token still serves ordinary API calls ...
        let listed = reqwest::Client::new()
            .get(format!("{base}/v1/reviews"))
            .bearer_auth("service-secret")
            .send()
            .await
            .unwrap();
        assert_eq!(listed.status().as_u16(), 200);

        // ... but cannot decide a review, even with a self-declared actor header.
        let (status, body) = decide_with(&base, "service-secret", &[("X-AMAP-Actor", "cto")]).await;
        assert_eq!(status, 403, "{body}");

        // Unauthenticated, expired, wrong-audience and forged tokens are rejected outright.
        assert_eq!(decide_with(&base, "", &[]).await.0, 401);
        assert_eq!(
            decide_with(&base, &token(json!({ "exp": testing::now() - 60 })), &[])
                .await
                .0,
            401
        );
        assert_eq!(
            decide_with(&base, &token(json!({ "aud": "other" })), &[])
                .await
                .0,
            401
        );

        // A verified human without the reviewer role is denied by policy.
        let (status, body) = decide_with(
            &base,
            &token(json!({ "sub": "user-7", "realm_access": { "roles": ["viewer"] } })),
            &[],
        )
        .await;
        assert_eq!(status, 403, "{body}");
        assert!(body.contains("policy denied"), "{body}");

        // A verified human holding the role decides, and the ledger records who and how.
        let (status, body) = decide_with(&base, &token(json!({})), &[]).await;
        assert_eq!(status, 200, "{body}");
        let review = platform
            .knowledge
            .list_reviews()
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.id == "HITL-1")
            .unwrap();
        assert_eq!(review.status, ReviewStatus::Approved);
        assert_eq!(review.decided_by.as_deref(), Some("user-42"));
        assert_eq!(
            review.decided_via.as_deref(),
            Some(format!("oidc:{}", testing::ISSUER).as_str())
        );

        // A decided review is immutable.
        let (status, _) = decide_with(&base, &token(json!({})), &[]).await;
        assert_eq!(status, 409);
    }

    #[tokio::test]
    async fn any_verified_human_may_decide_when_no_role_is_configured() {
        let (app, _) = test_app(None).await;
        let base = serve(app).await;
        let (status, body) = decide_with(
            &base,
            &token(json!({ "sub": "user-9", "realm_access": { "roles": [] } })),
            &[],
        )
        .await;
        assert_eq!(status, 200, "{body}");
    }

    #[test]
    fn openapi_contract_covers_control_plane_routes() {
        let contract = include_str!("../../../openapi/amap.yaml");
        for path in [
            "/v1/specs:",
            "/v1/runs:",
            "/v1/runs/{id}:",
            "/v1/runs/{id}/events:",
            "/v1/runs/{id}/resume:",
            "/v1/functions:",
            "/v1/reviews:",
        ] {
            assert!(contract.contains(path), "missing OpenAPI path {path}");
        }
    }
}
