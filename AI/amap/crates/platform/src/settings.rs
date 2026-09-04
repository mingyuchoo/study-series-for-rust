//! Platform settings (figment: `config/amap.toml` + `AMAP_*` environment variables).
use figment::providers::{Env, Format, Serialized, Toml};
use figment::Figment;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    /// PostgreSQL URL; in-memory knowledge store when unset.
    pub database_url: Option<String>,
    /// NATS URL; in-memory bus when unset.
    pub nats_url: Option<String>,
    /// Evidence lake: local directory or `s3://bucket/prefix`.
    pub lake: String,
    /// Standalone LLM gateway URL; embedded gateway when unset.
    pub llm_gateway_url: Option<String>,
    /// Bearer token shared only with the standalone LLM gateway.
    pub llm_gateway_token: Option<String>,
    pub control_plane_listen: String,
    pub llm_gateway_listen: String,
    pub worker_listen: String,
    pub json_logs: bool,
    pub otlp_endpoint: Option<String>,
    /// Per-run LLM token budget (0 = unlimited).
    pub token_budget: u64,
    /// Bearer token required by the control-plane API.
    pub api_token: Option<String>,
    /// Shared token required on worker gRPC requests.
    pub worker_token: Option<String>,
    /// Operator-controlled gRPC endpoint; run specifications cannot override it.
    pub verifier_endpoint: Option<String>,
    /// Root containing run specifications accepted by the control plane.
    pub spec_root: PathBuf,
    /// Optional compiled web UI served by the control plane when index.html exists.
    pub web_dist: PathBuf,
    /// Filesystem boundary for commands executed by verification workers.
    pub worker_root: PathBuf,
    /// Comma-separated executable basenames accepted by workers.
    pub worker_allowed_executables: String,
    pub worker_artifact_max_bytes: usize,
    /// Explicit opt-out for local development only.
    pub insecure_dev: bool,
    pub worker_tls_cert: Option<PathBuf>,
    pub worker_tls_key: Option<PathBuf>,
    pub worker_tls_client_ca: Option<PathBuf>,
    pub verifier_tls_ca: Option<PathBuf>,
    pub verifier_tls_cert: Option<PathBuf>,
    pub verifier_tls_key: Option<PathBuf>,
    pub verifier_tls_domain: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            database_url: None,
            nats_url: None,
            lake: ".amap/lake".into(),
            llm_gateway_url: None,
            llm_gateway_token: None,
            control_plane_listen: "127.0.0.1:8080".into(),
            llm_gateway_listen: "127.0.0.1:8090".into(),
            worker_listen: "127.0.0.1:50051".into(),
            json_logs: false,
            otlp_endpoint: None,
            token_budget: 0,
            api_token: None,
            worker_token: None,
            verifier_endpoint: None,
            spec_root: ".".into(),
            web_dist: "web/dist".into(),
            worker_root: ".".into(),
            worker_allowed_executables: "python3,python,cargo,java,javac".into(),
            worker_artifact_max_bytes: 64 * 1024 * 1024,
            insecure_dev: false,
            worker_tls_cert: None,
            worker_tls_key: None,
            worker_tls_client_ca: None,
            verifier_tls_ca: None,
            verifier_tls_cert: None,
            verifier_tls_key: None,
            verifier_tls_domain: None,
        }
    }
}

impl Settings {
    pub fn load(path: Option<&str>) -> anyhow::Result<Self> {
        let mut f = Figment::from(Serialized::defaults(Settings::default()));
        let p = path.unwrap_or("config/amap.toml");
        if std::path::Path::new(p).exists() {
            f = f.merge(Toml::file(p));
        }
        Ok(f.merge(Env::prefixed("AMAP_")).extract()?)
    }
}
