//! Run specification file (`amap.toml` next to a modernization target).
use amap_domain::*;
use amap_orchestrator::RunConfig;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionSpec {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub priority: Priority,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequirementSpec {
    pub id: String,
    pub title: String,
    pub text: String,
    #[serde(default = "default_conf")]
    pub confidence: f64,
}
fn default_conf() -> f64 {
    0.8
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MockSpec {
    pub fixtures: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunSpec {
    pub function: FunctionSpec,
    #[serde(default)]
    pub requirements: Vec<RequirementSpec>,
    pub run: RunConfig,
    #[serde(default)]
    pub mock: MockSpec,
    #[serde(skip)]
    pub base_dir: PathBuf,
}

impl RunSpec {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut spec: RunSpec = toml::from_str(&text)?;
        let base = path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let base = std::fs::canonicalize(&base).unwrap_or(base);
        spec.base_dir = base.clone();
        let abs = |p: &PathBuf| {
            if p.is_absolute() {
                p.clone()
            } else {
                base.join(p)
            }
        };
        spec.run.source_root = abs(&spec.run.source_root);
        spec.run.workspace = abs(&spec.run.workspace);
        spec.run.documents = spec.run.documents.iter().map(abs).collect();
        spec.run.traces = spec.run.traces.as_ref().map(abs);
        for source in &mut spec.run.trace_sources {
            if let amap_orchestrator::TraceSource::File { path } = source {
                *path = abs(path);
            }
        }
        spec.run.comparator_specs = spec.run.comparator_specs.iter().map(abs).collect();
        for plugin in &mut spec.run.comparator_plugins {
            plugin.path = abs(&plugin.path);
        }
        spec.run.invariants = spec.run.invariants.as_ref().map(abs);
        spec.mock.fixtures = spec.mock.fixtures.as_ref().map(abs);
        // Commands may reference relative paths (legacy emulator); resolve `legacy/...` style args.
        let fix_cmd = |cmd: &mut Vec<String>| {
            for a in cmd.iter_mut().skip(1) {
                if !a.contains('{') && !a.starts_with('-') && base.join(&*a).exists() {
                    *a = base.join(&*a).display().to_string();
                }
            }
        };
        fix_cmd(&mut spec.run.next_command);
        if let Some(c) = spec.run.legacy_command.as_mut() {
            fix_cmd(c);
        }
        Ok(spec)
    }

    pub fn function(&self) -> BusinessFunction {
        BusinessFunction {
            id: FunctionId::new(&self.function.id),
            name: self.function.name.clone(),
            domain: self.function.domain.clone(),
            priority: self.function.priority,
            description: self.function.description.clone(),
        }
    }

    pub fn requirements(&self) -> Vec<Requirement> {
        self.requirements
            .iter()
            .map(|r| Requirement {
                id: RequirementId::new(&r.id),
                function_id: FunctionId::new(&self.function.id),
                title: r.title.clone(),
                text: r.text.clone(),
                confidence: r.confidence,
            })
            .collect()
    }
}
