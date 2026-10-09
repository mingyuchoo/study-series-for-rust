//! The composition root: concrete adapters meet application use cases here.
use super::Project;
use anyhow::Result;
use gm_application::{Activation,
                     BuildGeneration,
                     StoredGeneration,
                     activate_generation,
                     build_generation};
use gm_core::GenerationId;
use gm_runner::{Supervisor,
                artifacts::FileArtifactCollector,
                clock::SystemClock,
                exec::ShellExecutor,
                git::GitSourceControl,
                health::LocalHealthVerifier};
use gm_store::{FileRunState,
               ProjectLock};
use std::path::Path;

impl Project {
    pub fn supervisor(&self) -> Supervisor { Supervisor::new(FileRunState::new(self.store.layout().clone()), self.store.layout().log_file()) }

    pub fn build(&self, source: &Path, worktree: Option<&str>, note: Option<String>, lock: &ProjectLock) -> Result<StoredGeneration> {
        let repository = self.store.repository(lock)?;
        Ok(build_generation(
            BuildGeneration {
                source,
                worktree,
                note,
            },
            &self.config,
            &repository,
            &ShellExecutor,
            &GitSourceControl,
            &FileArtifactCollector,
            &SystemClock,
        )?)
    }

    pub fn activate(&self, id: GenerationId, reason: &str, lock: &ProjectLock) -> Result<Activation> {
        let repository = self.store.repository(lock)?;
        Ok(activate_generation(
            id,
            reason,
            &self.config,
            &repository,
            &self.supervisor(),
            &LocalHealthVerifier,
            &SystemClock,
        )?)
    }
}
