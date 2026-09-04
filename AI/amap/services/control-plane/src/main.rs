//! Modernization Control Plane (design §1): policy, quality gate, risk, evidence, audit and
//! HITL over a REST API. Hosts the orchestrator in modular-monolith mode.
mod auth;

use amap_domain::*;
use amap_knowledge::{LocalPathInput, ReviewStatus, RunInputs, WorkflowRun};
use amap_orchestrator::AgentContext;
#[allow(unused_imports)]
use amap_platform::{Platform, PlatformBuilder, RunSpec, Settings};
use amap_policy::{ActionContext, Principal};
use auth::{OidcConfig, OidcVerifier};
use axum::extract::{DefaultBodyLimit, Extension, Path, Query, State};
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use futures::{stream, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::convert::Infallible;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Component, Path as FilePath, PathBuf};
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
    /// Per-process secret used to bind short-lived preflight tokens to normalized inputs.
    preflight_secret: String,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StartRun {
    /// Path to an `amap.toml` run spec on the control-plane host.
    spec: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    source: Option<PathInput>,
    #[serde(default)]
    destination: Option<PathInput>,
    #[serde(default)]
    mock: bool,
    #[serde(default)]
    validation_token: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PathInput {
    #[serde(rename = "type")]
    kind: String,
    path: String,
}

#[derive(Clone, Debug, Serialize)]
struct SpecDefaults {
    source_path: String,
    destination_path: String,
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
    runnable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    unavailable_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    defaults: Option<SpecDefaults>,
}

fn discover_run_specs(root: &FilePath, execution_root: &FilePath) -> Vec<SpecSummary> {
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
            let source_path = worker_relative_path(execution_root, &spec.run.source_root);
            let destination_path = worker_relative_path(execution_root, &spec.run.workspace);
            let defaults = source_path
                .zip(destination_path)
                .map(|(source, destination)| SpecDefaults {
                    source_path: path_text(&source),
                    destination_path: path_text(&destination),
                });
            let runnable = defaults.is_some();
            specs.push(SpecSummary {
                path: relative.to_string_lossy().replace('\\', "/"),
                function_id: spec.function.id,
                name: spec.function.name,
                domain: spec.function.domain,
                priority: spec.function.priority,
                description: spec.function.description,
                mock_available: spec.mock.fixtures.is_some(),
                runnable,
                unavailable_reason: (!runnable).then(|| "path_outside_worker_root".into()),
                defaults,
            });
        }
    }
    specs.sort_by(|left, right| left.path.cmp(&right.path));
    specs
}

async fn list_specs(State(app): State<App>) -> Json<Vec<SpecSummary>> {
    Json(discover_run_specs(&app.spec_root, &app.execution_root))
}

const RUN_MARKER: &str = ".amap/run.json";
const SOURCE_EXTENSIONS: &[&str] = &[
    "cbl", "cob", "cpy", "rs", "js", "jsx", "ts", "tsx", "cs", "sql", "jcl",
];

#[derive(Clone, Debug, Serialize)]
struct FieldIssue {
    field: String,
    code: String,
    message: String,
}

impl FieldIssue {
    fn new(field: &str, code: &str, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug)]
struct RunRequestError {
    status: StatusCode,
    issue: FieldIssue,
}

impl RunRequestError {
    fn bad(field: &str, code: &str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            issue: FieldIssue::new(field, code, message),
        }
    }

    fn forbidden(field: &str, code: &str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            issue: FieldIssue::new(field, code, message),
        }
    }

    fn conflict(field: &str, code: &str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            issue: FieldIssue::new(field, code, message),
        }
    }

    fn response(self) -> (StatusCode, Json<ApiProblem>) {
        (
            self.status,
            Json(ApiProblem {
                code: self.issue.code,
                message: self.issue.message,
                field: Some(self.issue.field),
                request_id: format!("REQ-{}", uuid::Uuid::new_v4()),
            }),
        )
    }
}

#[derive(Debug, Serialize)]
struct ApiProblem {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    request_id: String,
}

#[derive(Debug)]
struct ValidatedRun {
    display_name: String,
    spec_path: PathBuf,
    spec: RunSpec,
    inputs: RunInputs,
    destination_path: PathBuf,
    source_file_count: usize,
    destination_state: String,
    warnings: Vec<FieldIssue>,
    mock: bool,
}

#[derive(Debug, Serialize)]
struct PreflightEffective {
    name: String,
    function_id: String,
    spec: String,
    source_path: String,
    destination_path: String,
    source_file_count: usize,
    destination_state: String,
}

#[derive(Debug, Serialize)]
struct PreflightResponse {
    valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    validation_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effective: Option<PreflightEffective>,
    errors: Vec<FieldIssue>,
    warnings: Vec<FieldIssue>,
}

#[derive(Debug, Deserialize)]
struct PathQuery {
    purpose: String,
    #[serde(default)]
    parent: String,
    #[serde(default)]
    search: String,
}

#[derive(Debug, Serialize)]
struct PathEntry {
    name: String,
    path: String,
    kind: &'static str,
    readable: bool,
    writable: bool,
}

#[derive(Debug, Serialize)]
struct PathListing {
    path: String,
    parent: Option<String>,
    entries: Vec<PathEntry>,
    truncated: bool,
}

fn path_text(path: &FilePath) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.is_empty() {
        ".".into()
    } else {
        text
    }
}

fn validate_relative_path(path: &FilePath, field: &str) -> Result<PathBuf, RunRequestError> {
    if path.as_os_str().is_empty() {
        return Err(RunRequestError::bad(
            field,
            "path_required",
            "경로를 입력하십시오.",
        ));
    }
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(RunRequestError::forbidden(
            field,
            "path_outside_worker_root",
            "worker_root 기준의 안전한 상대 경로만 사용할 수 있습니다.",
        ));
    }
    let normalized = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value),
            Component::CurDir => None,
            _ => None,
        })
        .collect::<PathBuf>();
    Ok(normalized)
}

fn ensure_no_symlink(
    root: &FilePath,
    relative: &FilePath,
    field: &str,
) -> Result<(), RunRequestError> {
    let mut candidate = root.to_path_buf();
    for component in relative.components() {
        if let Component::Normal(value) = component {
            candidate.push(value);
            match std::fs::symlink_metadata(&candidate) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(RunRequestError::forbidden(
                        field,
                        "symlink_not_allowed",
                        "심볼릭 링크가 포함된 경로는 사용할 수 없습니다.",
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(error) => {
                    return Err(RunRequestError::bad(
                        field,
                        "path_unavailable",
                        format!("경로를 확인할 수 없습니다: {error}"),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn resolve_worker_path(
    root: &FilePath,
    raw: &str,
    field: &str,
    must_exist: bool,
) -> Result<(PathBuf, PathBuf), RunRequestError> {
    let relative = validate_relative_path(FilePath::new(raw.trim()), field)?;
    ensure_no_symlink(root, &relative, field)?;
    let candidate = root.join(&relative);
    if must_exist {
        let resolved = std::fs::canonicalize(&candidate).map_err(|_| {
            RunRequestError::bad(field, "source_not_found", "원본 소스 디렉터리가 없습니다.")
        })?;
        if !resolved.starts_with(root) {
            return Err(RunRequestError::forbidden(
                field,
                "path_outside_worker_root",
                "허용된 실행 경계 밖의 경로입니다.",
            ));
        }
        if !resolved.is_dir() {
            return Err(RunRequestError::bad(
                field,
                "source_not_directory",
                "원본 소스는 디렉터리여야 합니다.",
            ));
        }
        std::fs::read_dir(&resolved).map_err(|_| {
            RunRequestError::bad(
                field,
                "source_not_readable",
                "원본 소스를 읽을 수 없습니다.",
            )
        })?;
        return Ok((relative, resolved));
    }

    let mut ancestor = candidate.as_path();
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or_else(|| {
            RunRequestError::bad(
                field,
                "destination_not_creatable",
                "결과 위치를 생성할 수 없습니다.",
            )
        })?;
    }
    let resolved_ancestor = std::fs::canonicalize(ancestor).map_err(|error| {
        RunRequestError::bad(
            field,
            "destination_not_creatable",
            format!("결과 위치의 상위 디렉터리를 확인할 수 없습니다: {error}"),
        )
    })?;
    if !resolved_ancestor.starts_with(root) || !resolved_ancestor.is_dir() {
        return Err(RunRequestError::forbidden(
            field,
            "path_outside_worker_root",
            "허용된 실행 경계 밖의 경로입니다.",
        ));
    }
    Ok((relative, candidate))
}

fn worker_relative_path(root: &FilePath, path: &FilePath) -> Option<PathBuf> {
    let candidate = if path.exists() {
        std::fs::canonicalize(path).ok()?
    } else {
        path.to_path_buf()
    };
    let relative = candidate.strip_prefix(root).ok()?;
    validate_relative_path(relative, "path").ok()
}

fn count_source_files(root: &FilePath) -> usize {
    let mut pending = vec![root.to_path_buf()];
    let mut count = 0usize;
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                pending.push(entry.path());
            } else if entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .map(|extension| {
                    SOURCE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
                })
                .unwrap_or(false)
            {
                count = count.saturating_add(1);
            }
            if count >= 100_000 {
                return count;
            }
        }
    }
    count
}

fn validate_name(value: String) -> Result<String, RunRequestError> {
    let value = value.trim().to_string();
    if value.is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err(RunRequestError::bad(
            "name",
            "invalid_name",
            "작업 이름은 제어문자 없이 1~80자로 입력하십시오.",
        ));
    }
    Ok(value)
}

fn resolve_spec_path(app: &App, raw: &FilePath) -> Result<PathBuf, RunRequestError> {
    let requested = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        app.spec_root.join(raw)
    };
    let requested = std::fs::canonicalize(&requested).map_err(|error| {
        RunRequestError::bad(
            "spec",
            "invalid_spec",
            format!("실행 명세가 올바르지 않습니다: {error}"),
        )
    })?;
    if !requested.starts_with(&app.spec_root) {
        return Err(RunRequestError::forbidden(
            "spec",
            "spec_outside_root",
            "실행 명세가 허용된 spec_root 밖에 있습니다.",
        ));
    }
    Ok(requested)
}

fn request_path_or_default(
    requested: &Option<PathInput>,
    default: &FilePath,
    app: &App,
    field: &str,
    must_exist: bool,
) -> Result<(PathBuf, PathBuf), RunRequestError> {
    if let Some(input) = requested {
        if input.kind != "local_path" {
            return Err(RunRequestError::bad(
                field,
                "unsupported_path_type",
                "현재는 local_path 입력만 지원합니다.",
            ));
        }
        resolve_worker_path(&app.execution_root, &input.path, field, must_exist)
    } else {
        let relative = worker_relative_path(&app.execution_root, default).ok_or_else(|| {
            RunRequestError::forbidden(
                field,
                "path_outside_worker_root",
                "명세의 기본 경로가 worker_root 밖에 있습니다.",
            )
        })?;
        resolve_worker_path(
            &app.execution_root,
            &path_text(&relative),
            field,
            must_exist,
        )
    }
}

fn validate_run_request(app: &App, req: &StartRun) -> Result<ValidatedRun, RunRequestError> {
    if req.mock && !app.insecure_dev {
        return Err(RunRequestError::forbidden(
            "mock",
            "mock_not_allowed",
            "운영 환경에서는 요청별 모의 실행을 사용할 수 없습니다.",
        ));
    }
    let spec_path = resolve_spec_path(app, FilePath::new(&req.spec))?;
    let mut spec = RunSpec::load(&spec_path).map_err(|error| {
        RunRequestError::bad(
            "spec",
            "invalid_spec",
            format!("실행 명세를 읽을 수 없습니다: {error}"),
        )
    })?;
    if spec.run.auto_approve_hitl && !app.insecure_dev {
        return Err(RunRequestError::forbidden(
            "spec",
            "auto_approve_not_allowed",
            "운영 환경에서는 HITL 자동 승인을 사용할 수 없습니다.",
        ));
    }

    let default_name = format!(
        "{} - {}",
        spec.function.name,
        chrono::Local::now().format("%Y-%m-%d")
    );
    let display_name = validate_name(req.name.clone().unwrap_or(default_name))?;
    let (source_relative, source_path) =
        request_path_or_default(&req.source, &spec.run.source_root, app, "source.path", true)?;
    let (destination_relative, destination_path) = request_path_or_default(
        &req.destination,
        &spec.run.workspace,
        app,
        "destination.path",
        false,
    )?;
    if source_path == destination_path
        || source_path.starts_with(&destination_path)
        || destination_path.starts_with(&source_path)
    {
        return Err(RunRequestError::conflict(
            "destination.path",
            "path_overlap",
            "결과 위치는 원본 소스와 같거나 상호 포함될 수 없습니다.",
        ));
    }
    let destination_state = if destination_path.exists() {
        let mut entries = std::fs::read_dir(&destination_path).map_err(|error| {
            RunRequestError::bad(
                "destination.path",
                "destination_not_creatable",
                format!("결과 위치를 확인할 수 없습니다: {error}"),
            )
        })?;
        if entries.next().is_some() {
            return Err(RunRequestError::conflict(
                "destination.path",
                "destination_conflict",
                "결과 위치가 비어 있지 않습니다. 새 디렉터리를 선택하십시오.",
            ));
        }
        "empty".to_string()
    } else {
        "will_create".to_string()
    };

    spec.apply_execution_paths(source_path.clone(), destination_path.clone());
    spec.run
        .validate_execution_boundary(&app.execution_root, &app.allowed_executables)
        .map_err(|error| RunRequestError::forbidden("spec", "execution_boundary", error))?;
    let source_file_count = count_source_files(&source_path);
    let mut warnings = Vec::new();
    if source_file_count == 0 {
        warnings.push(FieldIssue::new(
            "source.path",
            "no_supported_source_files",
            "지원되는 확장자의 소스 파일을 찾지 못했습니다.",
        ));
    }
    if !spec
        .run
        .next_command
        .iter()
        .any(|value| value.contains("{workspace}"))
    {
        warnings.push(FieldIssue::new(
            "spec",
            "command_may_ignore_override",
            "신규 시스템 실행 명령이 {workspace} 자리표시자를 사용하지 않습니다.",
        ));
    }
    if spec
        .run
        .legacy_command
        .as_ref()
        .is_some_and(|command| !command.iter().any(|value| value.contains("{source_root}")))
    {
        warnings.push(FieldIssue::new(
            "spec",
            "command_may_ignore_override",
            "레거시 실행 명령이 {source_root} 자리표시자를 사용하지 않습니다.",
        ));
    }
    Ok(ValidatedRun {
        display_name,
        spec_path,
        spec,
        inputs: RunInputs {
            source: LocalPathInput::local(source_relative),
            destination: LocalPathInput::local(destination_relative),
        },
        destination_path,
        source_file_count,
        destination_state,
        warnings,
        mock: req.mock,
    })
}

fn validation_signature(app: &App, actor: &AuthActor, expires: i64, run: &ValidatedRun) -> String {
    let mut digest = Sha256::new();
    digest.update(app.preflight_secret.as_bytes());
    digest.update([0]);
    digest.update(actor.id.as_bytes());
    digest.update([0]);
    digest.update(expires.to_string().as_bytes());
    digest.update([0]);
    digest.update(run.display_name.as_bytes());
    digest.update([0]);
    digest.update(run.spec_path.as_os_str().as_encoded_bytes());
    digest.update([0]);
    digest.update(run.inputs.source.path.as_os_str().as_encoded_bytes());
    digest.update([0]);
    digest.update(run.inputs.destination.path.as_os_str().as_encoded_bytes());
    digest.update([u8::from(run.mock)]);
    hex::encode(digest.finalize())
}

fn issue_validation_token(
    app: &App,
    actor: &AuthActor,
    run: &ValidatedRun,
) -> (String, chrono::DateTime<chrono::Utc>) {
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(5);
    let expires = expires_at.timestamp();
    (
        format!(
            "{expires}.{}",
            validation_signature(app, actor, expires, run)
        ),
        expires_at,
    )
}

fn verify_validation_token(
    app: &App,
    actor: &AuthActor,
    run: &ValidatedRun,
    token: &str,
) -> Result<(), RunRequestError> {
    let (expires, supplied) = token.split_once('.').ok_or_else(|| {
        RunRequestError::bad(
            "validation_token",
            "invalid_preflight",
            "사전 점검 결과가 올바르지 않습니다.",
        )
    })?;
    let expires = expires.parse::<i64>().map_err(|_| {
        RunRequestError::bad(
            "validation_token",
            "invalid_preflight",
            "사전 점검 결과가 올바르지 않습니다.",
        )
    })?;
    if expires < chrono::Utc::now().timestamp() {
        return Err(RunRequestError::bad(
            "validation_token",
            "preflight_expired",
            "사전 점검이 만료되었습니다. 다시 점검하십시오.",
        ));
    }
    let expected = validation_signature(app, actor, expires, run);
    if !constant_time_eq(&expected, supplied) {
        return Err(RunRequestError::bad(
            "validation_token",
            "preflight_changed",
            "입력값이 사전 점검 이후 변경되었습니다. 다시 점검하십시오.",
        ));
    }
    Ok(())
}

async fn preflight_run(
    State(app): State<App>,
    Extension(actor): Extension<AuthActor>,
    Json(req): Json<StartRun>,
) -> Json<PreflightResponse> {
    match validate_run_request(&app, &req) {
        Ok(run) => {
            let (validation_token, expires_at) = issue_validation_token(&app, &actor, &run);
            let spec = path_text(
                run.spec_path
                    .strip_prefix(&app.spec_root)
                    .unwrap_or(&run.spec_path),
            );
            Json(PreflightResponse {
                valid: true,
                validation_token: Some(validation_token),
                expires_at: Some(expires_at),
                effective: Some(PreflightEffective {
                    name: run.display_name,
                    function_id: run.spec.function.id,
                    spec,
                    source_path: path_text(&run.inputs.source.path),
                    destination_path: path_text(&run.inputs.destination.path),
                    source_file_count: run.source_file_count,
                    destination_state: run.destination_state,
                }),
                errors: vec![],
                warnings: run.warnings,
            })
        }
        Err(error) => Json(PreflightResponse {
            valid: false,
            validation_token: None,
            expires_at: None,
            effective: None,
            errors: vec![error.issue],
            warnings: vec![],
        }),
    }
}

async fn list_paths(
    State(app): State<App>,
    Query(query): Query<PathQuery>,
) -> Result<Json<PathListing>, (StatusCode, Json<ApiProblem>)> {
    if query.purpose != "source" && query.purpose != "destination" {
        return Err(RunRequestError::bad(
            "purpose",
            "invalid_purpose",
            "purpose는 source 또는 destination이어야 합니다.",
        )
        .response());
    }
    let (relative, directory) = resolve_worker_path(
        &app.execution_root,
        if query.parent.trim().is_empty() {
            "."
        } else {
            &query.parent
        },
        "parent",
        true,
    )
    .map_err(RunRequestError::response)?;
    let search = query.search.trim().to_lowercase();
    let mut entries = std::fs::read_dir(&directory)
        .map_err(|error| {
            RunRequestError::bad("parent", "path_unavailable", error.to_string()).response()
        })?
        .flatten()
        .filter_map(|entry| {
            let kind = entry.file_type().ok()?;
            if !kind.is_dir() || kind.is_symlink() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.')
                || (!search.is_empty() && !name.to_lowercase().contains(&search))
            {
                return None;
            }
            let path = relative.join(&name);
            let readable = std::fs::read_dir(entry.path()).is_ok();
            let writable = !entry.metadata().ok()?.permissions().readonly();
            Some(PathEntry {
                name,
                path: path_text(&path),
                kind: "directory",
                readable,
                writable,
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.name.to_lowercase());
    let truncated = entries.len() > 200;
    entries.truncate(200);
    let parent = relative.parent().map(path_text);
    Ok(Json(PathListing {
        path: path_text(&relative),
        parent,
        entries,
        truncated,
    }))
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

fn reserve_destination(path: &FilePath, run_id: &str, actor: &str) -> Result<(), RunRequestError> {
    std::fs::create_dir_all(path).map_err(|error| {
        RunRequestError::bad(
            "destination.path",
            "destination_not_creatable",
            format!("결과 위치를 생성할 수 없습니다: {error}"),
        )
    })?;
    let marker_path = path.join(RUN_MARKER);
    if let Some(parent) = marker_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            RunRequestError::bad(
                "destination.path",
                "destination_not_creatable",
                format!("결과 위치 메타데이터 디렉터리를 만들 수 없습니다: {error}"),
            )
        })?;
    }
    let mut marker = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker_path)
        .map_err(|_| {
            RunRequestError::conflict(
                "destination.path",
                "destination_conflict",
                "결과 위치가 다른 실행에 의해 사용 중입니다.",
            )
        })?;
    let body = serde_json::to_vec_pretty(&json!({
        "run_id": run_id,
        "created_by": actor,
        "created_at": chrono::Utc::now(),
    }))
    .map_err(|error| {
        RunRequestError::bad(
            "destination.path",
            "destination_not_creatable",
            format!("결과 위치 메타데이터를 만들 수 없습니다: {error}"),
        )
    })?;
    marker.write_all(&body).map_err(|error| {
        RunRequestError::bad(
            "destination.path",
            "destination_not_creatable",
            format!("결과 위치 메타데이터를 기록할 수 없습니다: {error}"),
        )
    })
}

fn present_run(app: &App, mut run: WorkflowRun) -> WorkflowRun {
    if run.display_name.trim().is_empty() {
        run.display_name = run.function_id.as_str().to_string();
    }
    if let Ok(relative) = run.spec_path.strip_prefix(&app.spec_root) {
        run.spec_path = relative.to_path_buf();
    }
    run
}

fn internal_problem(error: impl std::fmt::Display) -> (StatusCode, Json<ApiProblem>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiProblem {
            code: "internal_error".into(),
            message: error.to_string(),
            field: None,
            request_id: format!("REQ-{}", uuid::Uuid::new_v4()),
        }),
    )
}

async fn start_run(
    State(app): State<App>,
    Extension(actor): Extension<AuthActor>,
    Json(req): Json<StartRun>,
) -> Result<Json<WorkflowRun>, (StatusCode, Json<ApiProblem>)> {
    let validated = validate_run_request(&app, &req).map_err(RunRequestError::response)?;
    if let Some(token) = &req.validation_token {
        verify_validation_token(&app, &actor, &validated, token)
            .map_err(RunRequestError::response)?;
    }

    // Mock runs share the platform's stores but answer LLM calls from the spec's fixtures.
    let platform = if req.mock || app.platform.mock {
        Arc::new(
            app.platform
                .with_mock_fixtures(validated.spec.mock.fixtures.as_deref()),
        )
    } else {
        app.platform.clone()
    };
    let run_id = platform.ids.next("RUN");
    reserve_destination(&validated.destination_path, &run_id, &actor.id)
        .map_err(RunRequestError::response)?;
    let ctx = platform
        .context_for(&validated.spec, Some(run_id))
        .await
        .map_err(internal_problem)?;
    let now = platform.clock.now();
    let state = WorkflowRun {
        id: ctx.run_id.0.clone(),
        display_name: validated.display_name,
        function_id: ctx.function_id.clone(),
        status: "running".into(),
        spec_path: validated.spec_path,
        inputs: Some(validated.inputs),
        mock: platform.mock,
        started_by: actor.id,
        started_at: now,
        updated_at: now,
        outcome: None,
        checkpoint: json!({}),
    };
    app.platform
        .knowledge
        .upsert_workflow_run(state.clone())
        .await
        .map_err(internal_problem)?;
    platform
        .bus
        .publish(amap_orchestrator::Event {
            subject: "run.created".into(),
            run_id: state.id.clone(),
            function_id: state.function_id.as_str().to_string(),
            payload: json!({
                "display_name": state.display_name,
                "spec": path_text(state.spec_path.strip_prefix(&app.spec_root).unwrap_or(&state.spec_path)),
                "source_path": state.inputs.as_ref().map(|inputs| path_text(&inputs.source.path)),
                "destination_path": state.inputs.as_ref().map(|inputs| path_text(&inputs.destination.path)),
                "mock": state.mock,
                "started_by": state.started_by,
            }),
            at: now,
        })
        .await;
    launch_run(platform, ctx);
    Ok(Json(present_run(&app, state)))
}

async fn list_runs(State(app): State<App>) -> ApiResult<Vec<WorkflowRun>> {
    let runs = app
        .platform
        .knowledge
        .list_workflow_runs()
        .await
        .map_err(internal)?;
    Ok(Json(
        runs.into_iter().map(|run| present_run(&app, run)).collect(),
    ))
}

async fn get_run(State(app): State<App>, Path(id): Path<String>) -> ApiResult<WorkflowRun> {
    app.platform
        .knowledge
        .get_workflow_run(&id)
        .await
        .map_err(internal)?
        .map(|run| Json(present_run(&app, run)))
        .ok_or((StatusCode::NOT_FOUND, "run not found".into()))
}

async fn resume_run(
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<Json<WorkflowRun>, (StatusCode, Json<ApiProblem>)> {
    let mut state = app
        .platform
        .knowledge
        .get_workflow_run(&id)
        .await
        .map_err(internal_problem)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiProblem {
                    code: "run_not_found".into(),
                    message: "실행을 찾을 수 없습니다.".into(),
                    field: None,
                    request_id: format!("REQ-{}", uuid::Uuid::new_v4()),
                }),
            )
        })?;
    if state.status != "halted" {
        return Err(RunRequestError::conflict(
            "status",
            "run_not_halted",
            format!(
                "중단된 실행만 재개할 수 있습니다. 현재 상태: {}",
                state.status
            ),
        )
        .response());
    }
    let reviews = app
        .platform
        .knowledge
        .list_reviews()
        .await
        .map_err(internal_problem)?;
    if reviews.iter().any(|review| {
        review.run_id.as_ref().map(|run| run.as_str()) == Some(id.as_str())
            && review.status == ReviewStatus::Rejected
    }) {
        return Err(RunRequestError::conflict(
            "review",
            "review_rejected",
            "HITL 검토가 거절되어 실행을 재개할 수 없습니다.",
        )
        .response());
    }
    if !reviews.iter().any(|review| {
        review.run_id.as_ref().map(|run| run.as_str()) == Some(id.as_str())
            && review.status == ReviewStatus::Approved
    }) {
        return Err(RunRequestError::conflict(
            "review",
            "review_required",
            "실행을 재개하려면 승인된 HITL 검토가 필요합니다.",
        )
        .response());
    }
    let spec_path = resolve_spec_path(&app, &state.spec_path).map_err(RunRequestError::response)?;
    let mut spec = RunSpec::load(&spec_path).map_err(|error| {
        RunRequestError::bad("spec", "invalid_spec", error.to_string()).response()
    })?;
    if let Some(inputs) = &state.inputs {
        if inputs.source.kind != "local_path" || inputs.destination.kind != "local_path" {
            return Err(RunRequestError::conflict(
                "inputs",
                "run_inputs_changed",
                "저장된 실행 입력 형식을 더 이상 사용할 수 없습니다.",
            )
            .response());
        }
        let (_, source) = resolve_worker_path(
            &app.execution_root,
            &path_text(&inputs.source.path),
            "source.path",
            true,
        )
        .map_err(RunRequestError::response)?;
        let (_, destination) = resolve_worker_path(
            &app.execution_root,
            &path_text(&inputs.destination.path),
            "destination.path",
            false,
        )
        .map_err(RunRequestError::response)?;
        let marker = std::fs::read_to_string(destination.join(RUN_MARKER)).map_err(|_| {
            RunRequestError::conflict(
                "destination.path",
                "run_inputs_changed",
                "결과 위치의 실행 소유권 정보를 확인할 수 없습니다.",
            )
            .response()
        })?;
        let marker: Value = serde_json::from_str(&marker).map_err(|_| {
            RunRequestError::conflict(
                "destination.path",
                "run_inputs_changed",
                "결과 위치의 실행 소유권 정보가 손상되었습니다.",
            )
            .response()
        })?;
        if marker.get("run_id").and_then(Value::as_str) != Some(state.id.as_str()) {
            return Err(RunRequestError::conflict(
                "destination.path",
                "run_inputs_changed",
                "결과 위치가 다른 실행에 속합니다.",
            )
            .response());
        }
        spec.apply_execution_paths(source, destination);
    }
    spec.run
        .validate_execution_boundary(&app.execution_root, &app.allowed_executables)
        .map_err(|error| {
            RunRequestError::forbidden("spec", "execution_boundary", error).response()
        })?;
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
        .map_err(internal_problem)?;
    ctx.inputs = state.checkpoint.clone();
    state.status = "running".into();
    state.updated_at = platform.clock.now();
    state.outcome = None;
    app.platform
        .knowledge
        .upsert_workflow_run(state.clone())
        .await
        .map_err(internal_problem)?;
    launch_run(platform, ctx);
    Ok(Json(present_run(&app, state)))
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
        preflight_secret: uuid::Uuid::new_v4().to_string(),
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
        .route("/v1/paths", get(list_paths))
        .route("/v1/runs/preflight", post(preflight_run))
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
        let specs = discover_run_specs(root, root);
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
            preflight_secret: "test-preflight-secret".into(),
        };
        (app, platform)
    }

    fn write_test_spec(root: &FilePath) {
        std::fs::create_dir_all(root.join("defaults/source")).unwrap();
        std::fs::write(root.join("defaults/source/legacy.rs"), "fn legacy() {}\n").unwrap();
        std::fs::write(
            root.join("amap.toml"),
            r#"
[function]
id = "FN-TEST-1"
name = "Test modernization"
domain = "Test"
priority = "P1"

[run]
source_root = "defaults/source"
workspace = "defaults/output"
next_command = ["python3", "{workspace}/main.py"]
"#,
        )
        .unwrap();
    }

    #[test]
    fn run_path_validation_rejects_escape_and_symlink_free_source_is_resolved() {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(scratch.path().join("source")).unwrap();
        let root = std::fs::canonicalize(scratch.path()).unwrap();

        let (relative, resolved) =
            resolve_worker_path(&root, "source", "source.path", true).unwrap();
        assert_eq!(relative, PathBuf::from("source"));
        assert_eq!(resolved, root.join("source"));

        let error = resolve_worker_path(&root, "../outside", "source.path", true).unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert_eq!(error.issue.code, "path_outside_worker_root");
    }

    #[cfg(unix)]
    #[test]
    fn run_path_validation_rejects_symlinks() {
        use std::os::unix::fs::symlink;

        let scratch = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), scratch.path().join("linked")).unwrap();
        let root = std::fs::canonicalize(scratch.path()).unwrap();
        let error = resolve_worker_path(&root, "linked", "source.path", true).unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert_eq!(error.issue.code, "symlink_not_allowed");
    }

    #[tokio::test]
    async fn run_request_overrides_spec_paths_and_rejects_overlap() {
        let scratch = tempfile::tempdir().unwrap();
        write_test_spec(scratch.path());
        std::fs::create_dir_all(scratch.path().join("selected/source")).unwrap();
        std::fs::write(
            scratch.path().join("selected/source/input.rs"),
            "fn main() {}\n",
        )
        .unwrap();
        let (mut app, _) = test_app(None).await;
        app.spec_root = std::fs::canonicalize(scratch.path()).unwrap();
        app.execution_root = app.spec_root.clone();
        app.allowed_executables = HashSet::from(["python3".to_string()]);

        let request = StartRun {
            spec: "amap.toml".into(),
            name: Some("첫 번째 현대화".into()),
            source: Some(PathInput {
                kind: "local_path".into(),
                path: "selected/source".into(),
            }),
            destination: Some(PathInput {
                kind: "local_path".into(),
                path: "selected/output".into(),
            }),
            mock: false,
            validation_token: None,
        };
        let validated = validate_run_request(&app, &request).unwrap();
        assert_eq!(validated.display_name, "첫 번째 현대화");
        assert_eq!(
            validated.spec.run.source_root,
            app.execution_root.join("selected/source")
        );
        assert_eq!(
            validated.spec.run.workspace,
            app.execution_root.join("selected/output")
        );
        assert_eq!(validated.source_file_count, 1);

        let overlap = StartRun {
            destination: Some(PathInput {
                kind: "local_path".into(),
                path: "selected/source/output".into(),
            }),
            ..request.clone()
        };
        let error = validate_run_request(&app, &overlap).unwrap_err();
        assert_eq!(error.status, StatusCode::CONFLICT);
        assert_eq!(error.issue.code, "path_overlap");

        std::fs::create_dir_all(scratch.path().join("selected/occupied")).unwrap();
        std::fs::write(
            scratch.path().join("selected/occupied/existing.txt"),
            "owned",
        )
        .unwrap();
        let occupied = StartRun {
            destination: Some(PathInput {
                kind: "local_path".into(),
                path: "selected/occupied".into(),
            }),
            ..request
        };
        let error = validate_run_request(&app, &occupied).unwrap_err();
        assert_eq!(error.status, StatusCode::CONFLICT);
        assert_eq!(error.issue.code, "destination_conflict");
    }

    #[test]
    fn destination_reservation_records_run_ownership() {
        let scratch = tempfile::tempdir().unwrap();
        let destination = scratch.path().join("output");
        reserve_destination(&destination, "RUN-1", "operator-1").unwrap();
        let marker: Value =
            serde_json::from_str(&std::fs::read_to_string(destination.join(RUN_MARKER)).unwrap())
                .unwrap();
        assert_eq!(marker["run_id"], "RUN-1");
        assert_eq!(marker["created_by"], "operator-1");
        let error = reserve_destination(&destination, "RUN-2", "operator-2").unwrap_err();
        assert_eq!(error.issue.code, "destination_conflict");
    }

    #[tokio::test]
    async fn preflight_token_is_bound_to_normalized_inputs() {
        let scratch = tempfile::tempdir().unwrap();
        write_test_spec(scratch.path());
        let (mut app, _) = test_app(None).await;
        app.spec_root = std::fs::canonicalize(scratch.path()).unwrap();
        app.execution_root = app.spec_root.clone();
        app.allowed_executables = HashSet::from(["python3".to_string()]);
        let actor = AuthActor {
            id: "operator-1".into(),
            method: AuthMethod::Oidc,
            roles: vec![],
            issuer: Some("test".into()),
        };
        let request = StartRun {
            spec: "amap.toml".into(),
            name: Some("Token test".into()),
            source: None,
            destination: None,
            mock: false,
            validation_token: None,
        };
        let validated = validate_run_request(&app, &request).unwrap();
        let (token, _) = issue_validation_token(&app, &actor, &validated);
        verify_validation_token(&app, &actor, &validated, &token).unwrap();

        let mut changed = validated;
        changed.display_name = "Changed".into();
        let error = verify_validation_token(&app, &actor, &changed, &token).unwrap_err();
        assert_eq!(error.issue.code, "preflight_changed");
    }

    #[test]
    fn legacy_workflow_run_deserializes_with_new_fields() {
        let run: WorkflowRun = serde_json::from_value(json!({
            "id": "RUN-OLD",
            "function_id": "FN-OLD",
            "status": "failed",
            "spec_path": "amap.toml",
            "mock": false,
            "started_at": "2026-09-05T00:00:00Z",
            "updated_at": "2026-09-05T00:00:00Z",
            "checkpoint": {}
        }))
        .unwrap();
        assert!(run.display_name.is_empty());
        assert!(run.inputs.is_none());
        assert!(run.started_by.is_empty());
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
            "/v1/paths:",
            "/v1/runs:",
            "/v1/runs/preflight:",
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
