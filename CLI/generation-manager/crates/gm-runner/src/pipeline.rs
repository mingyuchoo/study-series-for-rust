use crate::{artifacts,
            exec::run_stage,
            git,
            health::wait_until_healthy,
            supervisor::Supervisor};
use chrono::Utc;
use gm_core::{config::Config,
              error::Result,
              generation::{Generation,
                           GenerationId,
                           GenerationStatus},
              run::{RunSource,
                    RunState}};
use gm_store::{GenerationEntry,
               ProjectLock,
               Store};
use std::{path::Path,
          time::Duration};

/// What happened when a generation was switched in.
#[derive(Debug, Clone)]
pub enum Activation {
    /// The new generation is live and passed its health check.
    Healthy { id: GenerationId, pid: i32 },
    /// The new generation failed verification and was backed out.
    RolledBack {
        failed: GenerationId,
        restored: Option<GenerationId>,
        /// Whether the restored generation itself came back healthy.
        restored_healthy: bool,
        reason: String,
    },
}

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

    fn stop_timeout(&self) -> Duration { Duration::from_secs(self.config.run.stop_timeout_secs) }

    /// Build and test `source`, then freeze the declared artifacts into a new
    /// generation. Nothing is published if any stage fails, so a broken build
    /// never occupies a generation number.
    pub fn build(&self, source: &Path, worktree: Option<&str>, note: Option<String>) -> Result<GenerationEntry> {
        if !self.config.build.is_empty() {
            run_stage("build", &self.config.build.cmd, source, &self.config.build.env)?;
        }
        if !self.config.test.is_empty() {
            run_stage("test", &self.config.test.cmd, source, &self.config.test.env)?;
        }

        let commit = git::head_commit(source);
        let dirty = git::is_dirty(source);

        let staged = self.store.stage(commit.as_deref())?;
        let collected = match artifacts::collect(source, &self.config.artifacts.include, &staged.payload) {
            | Ok(list) => list,
            | Err(err) => {
                self.store.discard(staged);
                return Err(err);
            },
        };

        let meta = Generation {
            id: staged.id,
            commit,
            worktree: worktree.map(str::to_string),
            dirty,
            built_at: Utc::now(),
            status: GenerationStatus::Built,
            artifacts: collected,
            note,
        };
        self.store.commit(staged, meta)
    }

    /// Point `current` at `id`, restart the service, and verify it. On failure
    /// the previous generation is restored before returning.
    pub fn activate(&self, id: GenerationId, reason: &str) -> Result<Activation> {
        let entry = self.store.get(id)?;
        let previous = self.store.current_id()?;
        let supervisor = self.supervisor();

        supervisor.stop_if_running(self.stop_timeout())?;
        self.store.switch(id, reason)?;

        let source = RunSource::Generation {
            id,
        };
        let start_error = match supervisor.start_detached(&self.config.run, &entry.payload(), source.clone()) {
            | Ok(state) => match wait_until_healthy(&self.config.health, &entry.payload()) {
                | Ok(()) => {
                    self.store.set_status(id, GenerationStatus::Healthy)?;
                    return Ok(Activation::Healthy {
                        id,
                        pid: state.pid,
                    });
                },
                | Err(err) => err.to_string(),
            },
            | Err(err) => err.to_string(),
        };

        // Verification failed: mark, stop, and back out.
        self.store.set_status(id, GenerationStatus::Rejected)?;
        supervisor.stop_if_running(self.stop_timeout())?;

        let (restored, restored_healthy) = match previous {
            | Some(prev) if prev != id => {
                self.store.switch(prev, "auto-rollback after failed health check")?;
                let prev_entry = self.store.get(prev)?;
                // Wait for the restored generation too, so the command only
                // returns once the service is answering again. A failure here is
                // reported rather than raised: the operator needs to see both
                // facts, not just the second one.
                let healthy = supervisor
                    .start_detached(
                        &self.config.run,
                        &prev_entry.payload(),
                        RunSource::Generation {
                            id: prev,
                        },
                    )
                    .is_ok()
                    && wait_until_healthy(&self.config.health, &prev_entry.payload()).is_ok();
                (Some(prev), healthy)
            },
            | _ => (None, false),
        };

        Ok(Activation::RolledBack {
            failed: id,
            restored,
            restored_healthy,
            reason: start_error,
        })
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
