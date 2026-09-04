//! Verification workers: gRPC (Tonic) services that run one or more verification engines.
//! The orchestrator dispatches to them when `verifier_endpoint` is configured.
use amap_domain::*;
use amap_orchestrator::proto::verification_server::{Verification, VerificationServer};
use amap_orchestrator::proto::{HealthRequest, HealthResponse, VerifyRequest, VerifyResponse};
use amap_orchestrator::RunConfig;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use tonic::{Request, Response, Status};

pub struct Worker {
    pub name: String,
    pub kinds: Vec<VerificationKind>,
    token: Option<String>,
    root: PathBuf,
    allowed_executables: HashSet<String>,
    require_artifacts: bool,
    artifact_max_bytes: usize,
}

fn authorized<T>(request: &Request<T>, expected: Option<&str>) -> bool {
    use subtle::ConstantTimeEq;
    let Some(expected) = expected else {
        return true;
    };
    let supplied = request
        .metadata()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    expected.as_bytes().ct_eq(supplied.as_bytes()).into()
}

fn validate_config(worker: &Worker, config: &RunConfig) -> Result<(), Status> {
    config
        .validate_execution_boundary(&worker.root, &worker.allowed_executables)
        .map_err(Status::permission_denied)
}

fn safe_relative(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn rebase_path(
    path: &Path,
    old_source: &Path,
    old_workspace: &Path,
    new_source: &Path,
    new_workspace: &Path,
    exact: &HashMap<PathBuf, PathBuf>,
) -> PathBuf {
    if let Ok(relative) = path.strip_prefix(old_source) {
        new_source.join(relative)
    } else if let Ok(relative) = path.strip_prefix(old_workspace) {
        new_workspace.join(relative)
    } else {
        exact
            .get(path)
            .cloned()
            .unwrap_or_else(|| path.to_path_buf())
    }
}

fn rebase_command(
    command: &mut [String],
    old_source: &Path,
    old_workspace: &Path,
    new_source: &Path,
    new_workspace: &Path,
    exact: &HashMap<PathBuf, PathBuf>,
) {
    for argument in command {
        if argument.contains('{') {
            continue;
        }
        let path = Path::new(argument);
        if path.is_absolute() {
            *argument = rebase_path(
                path,
                old_source,
                old_workspace,
                new_source,
                new_workspace,
                exact,
            )
            .to_string_lossy()
            .to_string();
        }
    }
}

fn materialize(
    worker: &Worker,
    artifacts: &[amap_orchestrator::proto::Artifact],
    config: &mut RunConfig,
) -> Result<tempfile::TempDir, Status> {
    if artifacts.is_empty() && worker.require_artifacts {
        return Err(Status::failed_precondition(
            "a self-contained artifact bundle is required",
        ));
    }
    let sandbox = tempfile::Builder::new()
        .prefix("amap-worker-")
        .tempdir_in(&worker.root)
        .map_err(|error| Status::internal(error.to_string()))?;
    let old_source = config.source_root.clone();
    let old_workspace = config.workspace.clone();
    let new_source = sandbox.path().join("source");
    let new_workspace = sandbox.path().join("workspace");
    std::fs::create_dir_all(&new_source).map_err(|e| Status::internal(e.to_string()))?;
    std::fs::create_dir_all(&new_workspace).map_err(|e| Status::internal(e.to_string()))?;

    let mut total = 0usize;
    let mut exact = HashMap::new();
    let mut destinations = HashSet::new();
    for artifact in artifacts {
        total = total.saturating_add(artifact.data.len());
        if total > worker.artifact_max_bytes {
            return Err(Status::resource_exhausted("artifact bundle is too large"));
        }
        let relative = Path::new(&artifact.path);
        if !safe_relative(relative) {
            return Err(Status::invalid_argument("unsafe artifact path"));
        }
        let base = match artifact.root.as_str() {
            "source" => &new_source,
            "workspace" => &new_workspace,
            "support" => sandbox.path(),
            _ => return Err(Status::invalid_argument("unknown artifact root")),
        };
        let destination = base.join(relative);
        if !destinations.insert(destination.clone()) {
            return Err(Status::invalid_argument("duplicate artifact path"));
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Status::internal(e.to_string()))?;
        }
        std::fs::write(&destination, &artifact.data)
            .map_err(|e| Status::internal(e.to_string()))?;
        exact.insert(PathBuf::from(&artifact.original_path), destination);
    }

    config.source_root = new_source.clone();
    config.workspace = new_workspace.clone();
    config.documents = config
        .documents
        .iter()
        .map(|path| {
            rebase_path(
                path,
                &old_source,
                &old_workspace,
                &new_source,
                &new_workspace,
                &exact,
            )
        })
        .collect();
    config.comparator_specs = config
        .comparator_specs
        .iter()
        .map(|path| {
            rebase_path(
                path,
                &old_source,
                &old_workspace,
                &new_source,
                &new_workspace,
                &exact,
            )
        })
        .collect();
    config.traces = config.traces.as_ref().map(|path| {
        rebase_path(
            path,
            &old_source,
            &old_workspace,
            &new_source,
            &new_workspace,
            &exact,
        )
    });
    for source in &mut config.trace_sources {
        if let amap_orchestrator::TraceSource::File { path } = source {
            *path = rebase_path(
                path,
                &old_source,
                &old_workspace,
                &new_source,
                &new_workspace,
                &exact,
            );
        }
    }
    config.invariants = config.invariants.as_ref().map(|path| {
        rebase_path(
            path,
            &old_source,
            &old_workspace,
            &new_source,
            &new_workspace,
            &exact,
        )
    });
    rebase_command(
        &mut config.next_command,
        &old_source,
        &old_workspace,
        &new_source,
        &new_workspace,
        &exact,
    );
    if let Some(command) = &mut config.legacy_command {
        rebase_command(
            command,
            &old_source,
            &old_workspace,
            &new_source,
            &new_workspace,
            &exact,
        );
    }
    Ok(sandbox)
}

fn kind_name(k: VerificationKind) -> String {
    serde_json::to_value(k)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

#[tonic::async_trait]
impl Verification for Worker {
    async fn verify(
        &self,
        req: Request<VerifyRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        if !authorized(&req, self.token.as_deref()) {
            return Err(Status::unauthenticated("invalid worker token"));
        }
        let req = req.into_inner();
        let kind: VerificationKind =
            serde_json::from_value(serde_json::Value::String(req.kind.clone()))
                .map_err(|e| Status::invalid_argument(e.to_string()))?;
        if !self.kinds.contains(&kind) {
            return Err(Status::failed_precondition(format!(
                "{} does not serve {}",
                self.name, req.kind
            )));
        }
        let scenarios: Vec<TestScenario> = serde_json::from_slice(&req.scenarios_json)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let mut config: RunConfig = serde_json::from_slice(&req.config_json)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let _sandbox = materialize(self, &req.artifacts, &mut config)?;
        validate_config(self, &config)?;
        let function_id = FunctionId::new(req.function_id);
        amap_telemetry::Metrics::global().inc("amap_worker_requests_total", &[("kind", &req.kind)]);
        tracing::info!(run = %req.run_id, function = %function_id, kind = %req.kind, scenarios = scenarios.len(), "verify request");
        let (results, metrics) =
            amap_agents::verification::run_engine(kind, &scenarios, &config, &function_id)
                .await
                .map_err(Status::internal)?;
        tracing::info!(kind = %req.kind, results = results.len(), passed = results.iter().filter(|r| r.passed).count(), "verify done");
        Ok(Response::new(VerifyResponse {
            results_json: serde_json::to_vec(&results).unwrap(),
            metrics_json: serde_json::to_vec(&metrics).unwrap(),
            worker: self.name.clone(),
        }))
    }

    async fn health(
        &self,
        request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        if !authorized(&request, self.token.as_deref()) {
            return Err(Status::unauthenticated("invalid worker token"));
        }
        Ok(Response::new(HealthResponse {
            status: "ok".into(),
            worker: self.name.clone(),
            kinds: self.kinds.iter().map(|k| kind_name(*k)).collect(),
        }))
    }
}

pub async fn serve(
    name: &str,
    kinds: Vec<VerificationKind>,
    listen: Option<String>,
) -> anyhow::Result<()> {
    let settings = amap_platform::Settings::load(None)?;
    if !settings.insecure_dev && settings.worker_token.is_none() {
        anyhow::bail!(
            "AMAP_WORKER_TOKEN is required; set AMAP_INSECURE_DEV=true only for isolated local development"
        );
    }
    amap_telemetry::init(&amap_telemetry::TelemetryConfig {
        service_name: name.into(),
        json: settings.json_logs,
        otlp_endpoint: settings.otlp_endpoint.clone(),
    });
    let addr = listen.unwrap_or(settings.worker_listen).parse()?;
    let root = std::fs::canonicalize(&settings.worker_root)?;
    let allowed_executables = settings
        .worker_allowed_executables
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect();
    tracing::info!(%addr, kinds = ?kinds, "{name} listening");
    let mut server = tonic::transport::Server::builder();
    match (
        &settings.worker_tls_cert,
        &settings.worker_tls_key,
        &settings.worker_tls_client_ca,
    ) {
        (Some(cert), Some(key), Some(client_ca)) => {
            use tonic::transport::{Certificate, Identity, ServerTlsConfig};
            server = server.tls_config(
                ServerTlsConfig::new()
                    .identity(Identity::from_pem(
                        tokio::fs::read(cert).await?,
                        tokio::fs::read(key).await?,
                    ))
                    .client_ca_root(Certificate::from_pem(tokio::fs::read(client_ca).await?)),
            )?;
        }
        (None, None, None) if settings.insecure_dev => {}
        (None, None, None) => {
            anyhow::bail!("worker mTLS certificate, key, and client CA are required")
        }
        _ => anyhow::bail!("worker TLS certificate, key, and client CA must be set together"),
    }
    server
        .add_service(
            VerificationServer::new(Worker {
                name: name.into(),
                kinds,
                token: settings.worker_token,
                root,
                allowed_executables,
                require_artifacts: !settings.insecure_dev,
                artifact_max_bytes: settings.worker_artifact_max_bytes,
            })
            .max_decoding_message_size(settings.worker_artifact_max_bytes),
        )
        .serve(addr)
        .await?;
    Ok(())
}
