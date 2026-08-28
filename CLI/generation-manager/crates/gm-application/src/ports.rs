use crate::PortResult;
use chrono::{DateTime,
             Utc};
use gm_core::{BuildStage,
              Generation,
              GenerationId,
              GenerationStatus,
              HealthCheck,
              RunSource,
              RunStage,
              RunState};
use std::{collections::BTreeMap,
          path::{Path,
                 PathBuf},
          time::Duration};

#[derive(Debug, Clone)]
pub struct StagedGeneration {
    pub id: GenerationId,
    pub payload: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StoredGeneration {
    pub meta: Generation,
    pub payload: PathBuf,
}

pub trait GenerationRepository {
    fn get(&self, id: GenerationId) -> PortResult<StoredGeneration>;
    fn current_id(&self) -> PortResult<Option<GenerationId>>;
    fn stage(&self, commit: Option<&str>) -> PortResult<StagedGeneration>;
    fn discard(&self, staged: StagedGeneration) -> PortResult<()>;
    fn commit(&self, staged: StagedGeneration, generation: Generation) -> PortResult<StoredGeneration>;
    fn set_status(&self, id: GenerationId, status: GenerationStatus) -> PortResult<()>;
    fn switch(&self, id: GenerationId, reason: &str, at: DateTime<Utc>) -> PortResult<()>;
}

pub trait StageExecutor {
    fn run(&self, name: &str, command: &str, cwd: &Path, env: &BTreeMap<String, String>) -> PortResult<()>;

    fn run_configured(&self, name: &str, stage: &BuildStage, cwd: &Path) -> PortResult<()> { self.run(name, &stage.cmd, cwd, &stage.env) }
}

pub trait SourceControl {
    fn head_commit(&self, cwd: &Path) -> Option<String>;
    fn is_dirty(&self, cwd: &Path) -> bool;
}

pub trait ArtifactCollector {
    fn collect(&self, source: &Path, includes: &[PathBuf], destination: &Path) -> PortResult<Vec<PathBuf>>;
}

pub trait ServiceRuntime {
    fn stop_if_running(&self, timeout: Duration) -> PortResult<Option<RunState>>;
    fn start_detached(&self, run: &RunStage, cwd: &Path, source: RunSource) -> PortResult<RunState>;
}

pub trait HealthVerifier {
    fn verify(&self, check: &HealthCheck, cwd: &Path) -> PortResult<()>;
}

pub trait Clock {
    fn now(&self) -> DateTime<Utc>;
}
