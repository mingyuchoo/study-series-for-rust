//! Observability bootstrap: `tracing` with an adapter boundary so the OpenTelemetry exporter
//! (still beta in Rust) can be swapped in without touching instrumentation code, plus a tiny
//! Prometheus text-format metrics registry.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub service_name: String,
    /// Emit JSON logs (for log pipelines) instead of human-readable output.
    #[serde(default)]
    pub json: bool,
    /// OTLP endpoint. When set, spans are annotated for export; the exporter adapter is pluggable.
    #[serde(default)]
    pub otlp_endpoint: Option<String>,
}

/// Initialise the global subscriber. Safe to call once per process.
pub fn init(cfg: &TelemetryConfig) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,hyper=warn,h2=warn,tantivy=warn"));
    let registry = tracing_subscriber::registry().with(filter);
    if cfg.json {
        let _ = registry.with(fmt::layer().json().with_current_span(true).with_writer(std::io::stderr)).try_init();
    } else {
        let _ = registry.with(fmt::layer().compact().with_writer(std::io::stderr)).try_init();
    }
    if let Some(ep) = &cfg.otlp_endpoint {
        tracing::info!(service = %cfg.service_name, endpoint = %ep, "OTLP export requested; attach exporter via the OTel adapter layer");
    }
    tracing::info!(service = %cfg.service_name, "telemetry initialised");
}

/// Minimal process-local metrics registry (counters + gauges) rendered in Prometheus exposition format.
#[derive(Default)]
pub struct Metrics {
    counters: Mutex<BTreeMap<String, AtomicU64>>,
    gauges: Mutex<BTreeMap<String, Mutex<f64>>>,
}

static GLOBAL: OnceLock<Metrics> = OnceLock::new();

impl Metrics {
    pub fn global() -> &'static Metrics {
        GLOBAL.get_or_init(Metrics::default)
    }

    pub fn inc(&self, name: &str, labels: &[(&str, &str)]) {
        let key = key(name, labels);
        let mut c = self.counters.lock().unwrap();
        c.entry(key).or_insert_with(|| AtomicU64::new(0)).fetch_add(1, Ordering::Relaxed);
    }

    pub fn add(&self, name: &str, labels: &[(&str, &str)], n: u64) {
        let key = key(name, labels);
        let mut c = self.counters.lock().unwrap();
        c.entry(key).or_insert_with(|| AtomicU64::new(0)).fetch_add(n, Ordering::Relaxed);
    }

    pub fn set(&self, name: &str, labels: &[(&str, &str)], v: f64) {
        let key = key(name, labels);
        let mut g = self.gauges.lock().unwrap();
        *g.entry(key).or_insert_with(|| Mutex::new(0.0)).get_mut().unwrap() = v;
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        for (k, v) in self.counters.lock().unwrap().iter() {
            out.push_str(&format!("{k} {}\n", v.load(Ordering::Relaxed)));
        }
        for (k, v) in self.gauges.lock().unwrap().iter() {
            out.push_str(&format!("{k} {}\n", v.lock().unwrap()));
        }
        out
    }
}

fn key(name: &str, labels: &[(&str, &str)]) -> String {
    if labels.is_empty() {
        name.to_string()
    } else {
        let l: Vec<String> = labels.iter().map(|(k, v)| format!("{k}=\"{v}\"")).collect();
        format!("{name}{{{}}}", l.join(","))
    }
}
