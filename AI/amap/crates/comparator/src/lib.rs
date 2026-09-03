//! Functional Equivalence Engine — deterministic comparators with data-driven specs.
//!
//! The final PASS/FAIL of a business output is *never* an LLM judgement: it is the
//! result of a [`Comparator`] chosen by a [`ComparatorSpec`] (YAML), optionally extended
//! by sandboxed WebAssembly plugins.

pub mod builtin;
pub mod engine;
pub mod spec;
#[cfg(feature = "wasm")]
pub mod wasm;

pub use amap_domain::Difference;
pub use engine::{ComparisonEngine, ComparisonResult};
pub use spec::{ComparatorKind, ComparatorSpec, FieldRule};

use serde_json::Value;
use std::collections::HashSet;
use std::sync::Mutex;

/// Contextual information passed to every comparator invocation.
pub struct ComparisonContext<'a> {
    pub path: &'a str,
    pub rule: &'a FieldRule,
    /// Values already seen for `unique: true` fields (shared across a run).
    pub seen_unique: &'a Mutex<HashSet<String>>,
}

/// A deterministic comparator. Implementations must be pure with respect to their inputs.
pub trait Comparator: Send + Sync {
    fn name(&self) -> &str;
    /// Returns `None` when equal, or a difference message when not.
    fn compare(&self, expected: &Value, actual: &Value, ctx: &ComparisonContext<'_>) -> Option<String>;
}

#[derive(Debug, thiserror::Error)]
pub enum ComparatorError {
    #[error("unknown comparator `{0}`")]
    Unknown(String),
    #[error("invalid spec: {0}")]
    Spec(String),
    #[error("plugin error: {0}")]
    Plugin(String),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
