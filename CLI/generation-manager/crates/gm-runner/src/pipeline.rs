use crate::{artifacts::FileArtifactCollector,
            error::Result,
            exec::{ShellExecutor,
                   run_stage},
            git::GitSourceControl,
            health::LocalHealthVerifier,
            supervisor::Supervisor};
use chrono::Utc;
use gm_application::{BuildGeneration,
                     Clock,
                     activate_generation,
                     build_generation};
use gm_core::{config::Config,
              generation::GenerationId,
              run::{RunSource,
                    RunState}};
use gm_store::{ProjectLock,
               Store};
use std::path::Path;

/// What happened when a development worktree was run directly.
#[derive(Debug, Clone)]
pub enum DevOutcome {
    /// Ran in the foreground and has already exited with this code.
    Exited { code: i32 },
    /// Left running in the background.
    Detached(RunState),
}

/// Ties the store, the build commands and the supervisor together.
pub struct Pipeline<'a> {
    store: &'a Store,
    config: &'a Config,
}

impl<'a> Pipeline<'a> {
    pub fn new(store: &'a Store, config: &'a Config) -> Pipeline<'a> {
        Pipeline {
            store,
            config,
        }
    }

    pub fn supervisor(&self) -> Supervisor { Supervisor::new(self.store.layout()) }

    /// Build and test `source`, then freeze the declared artifacts into a new
    /// generation. Nothing is published if any stage fails, so a broken build
    /// never occupies a generation number.
    pub fn build(&self, source: &Path, worktree: Option<&str>, note: Option<String>, lock: &ProjectLock) -> Result<gm_application::StoredGeneration> {
        let repository = self.store.repository(lock)?;
        Ok(build_generation(
            BuildGeneration {
                source,
                worktree,
                note,
            },
            self.config,
            &repository,
            &ShellExecutor,
            &GitSourceControl,
            &FileArtifactCollector,
            &SystemClock,
        )?)
    }

    /// Point `current` at `id`, restart the service, and verify it. On failure
    /// the previous generation is restored before returning.
    pub fn activate(&self, id: GenerationId, reason: &str, lock: &ProjectLock) -> Result<gm_application::Activation> {
        let supervisor = self.supervisor();
        let repository = self.store.repository(lock)?;
        Ok(activate_generation(
            id,
            reason,
            self.config,
            &repository,
            &supervisor,
            &LocalHealthVerifier,
            &SystemClock,
        )?)
    }

    /// Run a development worktree in place, without freezing a generation.
    ///
    /// The run command needs no separate configuration: a generation's payload
    /// mirrors the build directory's relative layout, so whatever `run.cmd`
    /// works from the store also works from the worktree it was built in.
    ///
    /// No health check and no automatic rollback here — the point of this mode
    /// is to try code that may not work yet.
    /// `lock` is released as soon as the service slot is taken: a foreground
    /// run lasts as long as the developer keeps it open, and holding the
    /// project lock for all of that would block every other command in the
    /// meantime.
    pub fn dev_run(&self, name: &str, path: &Path, build: bool, detach: bool, lock: ProjectLock) -> Result<DevOutcome> {
        if build && !self.config.build.is_empty() {
            run_stage("build", &self.config.build.cmd, path, &self.config.build.env)?;
        }

        let supervisor = self.supervisor();
        let source = RunSource::Worktree {
            name: name.to_string(),
        };

        if detach {
            let state = supervisor.start_detached(&self.config.run, path, source)?;
            drop(lock);
            Ok(DevOutcome::Detached(state))
        } else {
            let running = supervisor.spawn_foreground(&self.config.run, path, source)?;
            drop(lock);
            Ok(DevOutcome::Exited {
                code: running.wait()?,
            })
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> chrono::DateTime<Utc> { Utc::now() }
}
