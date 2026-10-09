use crate::{Clock,
            Error,
            GenerationRepository,
            HealthVerifier,
            Result,
            ServiceRuntime};
use gm_core::{Config,
              GenerationId,
              GenerationStatus,
              RunSource,
              activation_restore_target};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Activation {
    Healthy {
        id: GenerationId,
        pid: i32,
    },
    RolledBack {
        failed: GenerationId,
        restored: Option<GenerationId>,
        restored_healthy: bool,
        reason: String,
    },
}

pub fn activate_generation(
    id: GenerationId,
    reason: &str,
    config: &Config,
    repository: &impl GenerationRepository,
    runtime: &impl ServiceRuntime,
    health: &impl HealthVerifier,
    clock: &impl Clock,
) -> Result<Activation> {
    config.validate()?;
    let entry = Error::port("loading target generation", repository.get(id))?;
    let previous = Error::port("reading active generation", repository.current_id())?;
    let restored = activation_restore_target(previous, id);
    let previous_entry = restored
        .map(|previous| Error::port("loading previous generation", repository.get(previous)))
        .transpose()?;
    let timeout = Duration::from_secs(config.run.stop_timeout_secs);

    Error::port("stopping running service", runtime.stop_if_running(timeout))?;
    let attempt = (|| -> Result<_> {
        Error::port("switching active generation", repository.switch(id, reason, clock.now()))?;
        let state = Error::port(
            "starting generation",
            runtime.start_detached(
                &config.run,
                &entry.payload,
                RunSource::Generation {
                    id,
                },
            ),
        )?;
        Error::port("checking generation health", health.verify(&config.health, &entry.payload))?;
        Error::port("marking generation healthy", repository.set_status(id, GenerationStatus::Healthy))?;
        Ok(state)
    })();
    let mut failure = match attempt {
        | Ok(state) =>
            return Ok(Activation::Healthy {
                id,
                pid: state.pid,
            }),
        | Err(error) => error.to_string(),
    };

    if let Err(error) = repository.set_status(id, GenerationStatus::Rejected) {
        failure.push_str(&format!("; marking rejected failed: {error}"));
    }
    if let Err(error) = runtime.stop_if_running(timeout) {
        failure.push_str(&format!("; stopping rejected generation failed: {error}"));
    }

    let restored_healthy = if let Some(previous_entry) = previous_entry {
        let previous = previous_entry.meta.id;
        if let Err(error) = repository.switch(previous, "auto-rollback after failed health check", clock.now()) {
            failure.push_str(&format!("; recording rollback failed: {error}"));
            Error::port("restoring previous pointer", repository.restore(Some(previous)))?;
        }
        match runtime
            .start_detached(
                &config.run,
                &previous_entry.payload,
                RunSource::Generation {
                    id: previous,
                },
            )
            .and_then(|_| health.verify(&config.health, &previous_entry.payload))
        {
            | Ok(()) => true,
            | Err(error) => {
                failure.push_str(&format!("; restarting previous generation failed: {error}"));
                false
            },
        }
    } else {
        Error::port("clearing rejected pointer", repository.restore(None))?;
        false
    };

    Ok(Activation::RolledBack {
        failed: id,
        restored,
        restored_healthy,
        reason: failure,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoxError,
                Clock,
                GenerationRepository,
                HealthVerifier,
                PortResult,
                ServiceRuntime,
                StagedGeneration,
                StoredGeneration};
    use chrono::{DateTime,
                 TimeZone,
                 Utc};
    use gm_core::{Generation,
                  GenerationStatus,
                  Preset,
                  RunState};
    use std::{cell::{Cell,
                     RefCell},
              collections::BTreeMap,
              path::{Path,
                     PathBuf},
              time::Duration};

    struct Repository {
        entries: BTreeMap<GenerationId, StoredGeneration>,
        current: Cell<Option<GenerationId>>,
        statuses: RefCell<Vec<(GenerationId, GenerationStatus)>>,
        switches: RefCell<Vec<GenerationId>>,
        fail_switch: bool,
        fail_status: bool,
    }

    impl GenerationRepository for Repository {
        fn get(&self, id: GenerationId) -> PortResult<StoredGeneration> { self.entries.get(&id).cloned().ok_or_else(|| failure("missing generation")) }

        fn current_id(&self) -> PortResult<Option<GenerationId>> { Ok(self.current.get()) }

        fn stage(&self, _commit: Option<&str>) -> PortResult<StagedGeneration> { Err(failure("unused")) }

        fn discard(&self, _staged: StagedGeneration) -> PortResult<()> { Err(failure("unused")) }

        fn commit(&self, _staged: StagedGeneration, _generation: Generation) -> PortResult<StoredGeneration> { Err(failure("unused")) }

        fn set_status(&self, id: GenerationId, status: GenerationStatus) -> PortResult<()> {
            if self.fail_status {
                return Err(failure("status write failed"));
            }
            self.statuses.borrow_mut().push((id, status));
            Ok(())
        }

        fn switch(&self, id: GenerationId, _reason: &str, _at: DateTime<Utc>) -> PortResult<()> {
            self.current.set(Some(id));
            self.switches.borrow_mut().push(id);
            if self.fail_switch {
                return Err(failure("history write failed"));
            }
            Ok(())
        }

        fn restore(&self, id: Option<GenerationId>) -> PortResult<()> {
            self.current.set(id);
            Ok(())
        }
    }

    struct Runtime {
        fail_for: Option<GenerationId>,
        starts: RefCell<Vec<GenerationId>>,
    }

    impl ServiceRuntime for Runtime {
        fn stop_if_running(&self, _timeout: Duration) -> PortResult<Option<RunState>> { Ok(None) }

        fn start_detached(&self, _run: &gm_core::RunStage, _cwd: &Path, source: RunSource) -> PortResult<RunState> {
            let RunSource::Generation {
                id,
            } = source
            else {
                return Err(failure("unexpected worktree"));
            };
            self.starts.borrow_mut().push(id);
            if self.fail_for == Some(id) {
                return Err(failure("start failed"));
            }
            Ok(RunState {
                source: RunSource::Generation {
                    id,
                },
                pid: id.0 as i32,
                process_start: None,
                started_at: fixed_time(),
                detached: true,
            })
        }
    }

    struct Health {
        fail_for: Option<PathBuf>,
    }

    impl HealthVerifier for Health {
        fn verify(&self, _check: &gm_core::HealthCheck, cwd: &Path) -> PortResult<()> {
            if self.fail_for.as_deref() == Some(cwd) {
                Err(failure("unhealthy"))
            } else {
                Ok(())
            }
        }
    }

    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> { fixed_time() }
    }

    fn fixed_time() -> DateTime<Utc> { Utc.with_ymd_and_hms(2026, 8, 28, 0, 0, 0).unwrap() }

    fn failure(message: &str) -> BoxError { std::io::Error::other(message).into() }

    fn repository(current: Option<GenerationId>) -> Repository {
        let entries = [1, 2]
            .into_iter()
            .map(|value| {
                let id = GenerationId(value);
                let payload = PathBuf::from(format!("/generation/{value}"));
                (
                    id,
                    StoredGeneration {
                        meta: Generation {
                            id,
                            commit: None,
                            worktree: None,
                            dirty: false,
                            built_at: fixed_time(),
                            status: GenerationStatus::Built,
                            artifacts: Vec::new(),
                            note: None,
                        },
                        payload,
                    },
                )
            })
            .collect();
        Repository {
            entries,
            current: Cell::new(current),
            statuses: RefCell::new(Vec::new()),
            switches: RefCell::new(Vec::new()),
            fail_switch: false,
            fail_status: false,
        }
    }

    #[test]
    fn healthy_activation_updates_status_without_rollback() {
        let repository = repository(Some(GenerationId(1)));
        let runtime = Runtime {
            fail_for: None,
            starts: RefCell::new(Vec::new()),
        };
        let result = activate_generation(
            GenerationId(2),
            "test",
            &Preset::Generic.template("app"),
            &repository,
            &runtime,
            &Health {
                fail_for: None,
            },
            &FixedClock,
        )
        .unwrap();

        assert!(matches!(
            result,
            Activation::Healthy {
                id: GenerationId(2),
                ..
            }
        ));
        assert_eq!(repository.current.get(), Some(GenerationId(2)));
        assert_eq!(*repository.statuses.borrow(), vec![(GenerationId(2), GenerationStatus::Healthy)]);
    }

    #[test]
    fn failed_health_check_rejects_and_restores_the_previous_generation() {
        let repository = repository(Some(GenerationId(1)));
        let runtime = Runtime {
            fail_for: None,
            starts: RefCell::new(Vec::new()),
        };
        let result = activate_generation(
            GenerationId(2),
            "test",
            &Preset::Generic.template("app"),
            &repository,
            &runtime,
            &Health {
                fail_for: Some(PathBuf::from("/generation/2")),
            },
            &FixedClock,
        )
        .unwrap();

        assert!(matches!(
            result,
            Activation::RolledBack {
                failed: GenerationId(2),
                restored: Some(GenerationId(1)),
                restored_healthy: true,
                ..
            }
        ));
        assert_eq!(repository.current.get(), Some(GenerationId(1)));
        assert_eq!(*repository.switches.borrow(), vec![GenerationId(2), GenerationId(1)]);
        assert_eq!(*repository.statuses.borrow(), vec![(GenerationId(2), GenerationStatus::Rejected)]);
    }

    #[test]
    fn bookkeeping_failures_do_not_prevent_restoring_the_service() {
        for (fail_switch, fail_status) in [(true, false), (false, true)] {
            let mut repository = repository(Some(GenerationId(1)));
            repository.fail_switch = fail_switch;
            repository.fail_status = fail_status;
            let runtime = Runtime {
                fail_for: None,
                starts: RefCell::new(Vec::new()),
            };
            let result = activate_generation(
                GenerationId(2),
                "test",
                &Preset::Generic.template("app"),
                &repository,
                &runtime,
                &Health {
                    fail_for: Some(PathBuf::from("/generation/2")),
                },
                &FixedClock,
            )
            .unwrap();
            assert!(matches!(
                result,
                Activation::RolledBack {
                    restored: Some(GenerationId(1)),
                    restored_healthy: true,
                    ..
                }
            ));
            assert_eq!(repository.current.get(), Some(GenerationId(1)));
            assert_eq!(runtime.starts.borrow().last(), Some(&GenerationId(1)));
        }
    }

    #[test]
    fn failed_first_activation_clears_the_rejected_pointer() {
        let repository = repository(None);
        let runtime = Runtime {
            fail_for: Some(GenerationId(2)),
            starts: RefCell::new(Vec::new()),
        };
        activate_generation(
            GenerationId(2),
            "test",
            &Preset::Generic.template("app"),
            &repository,
            &runtime,
            &Health {
                fail_for: None,
            },
            &FixedClock,
        )
        .unwrap();
        assert_eq!(repository.current.get(), None);
    }
}
