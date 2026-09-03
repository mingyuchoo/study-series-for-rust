//! Platform settings (figment: `config/amap.toml` + `AMAP_*` environment variables).
use figment::providers::{Env, Format, Serialized, Toml};
use figment::Figment;
use serde::{Deserialize, Serialize};

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
    pub control_plane_listen: String,
    pub llm_gateway_listen: String,
    pub worker_listen: String,
    pub json_logs: bool,
    pub otlp_endpoint: Option<String>,
    /// Per-run LLM token budget (0 = unlimited).
    pub token_budget: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            database_url: None,
            nats_url: None,
            lake: ".amap/lake".into(),
            llm_gateway_url: None,
            control_plane_listen: "0.0.0.0:8080".into(),
            llm_gateway_listen: "0.0.0.0:8090".into(),
            worker_listen: "0.0.0.0:50051".into(),
            json_logs: false,
            otlp_endpoint: None,
            token_budget: 0,
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
