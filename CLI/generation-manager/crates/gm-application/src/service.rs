use crate::{Error,
            GenerationRepository,
            Result,
            ServiceRuntime};
use gm_core::{Config,
              RunSource,
              RunState};
use std::time::Duration;

pub struct StartedService {
    pub state: RunState,
    pub displaced: Option<RunState>,
}

/// Starting an active generation does not re-activate or re-check its health.
pub fn start_service(config: &Config, repository: &impl GenerationRepository, runtime: &impl ServiceRuntime, restart: bool) -> Result<StartedService> {
    config.validate()?;
    let id =
        Error::port("reading active generation", repository.current_id())?.ok_or_else(|| Error::Precondition("no generation is currently active".into()))?;
    // Resolve the payload before displacing the running service.
    let entry = Error::port("loading active generation", repository.get(id))?;
    let displaced = if restart {
        Error::port("stopping service", runtime.stop_if_running(Duration::from_secs(config.run.stop_timeout_secs)))?
    } else {
        None
    };
    let state = Error::port(
        "starting service",
        runtime.start_detached(
            &config.run,
            &entry.payload,
            RunSource::Generation {
                id,
            },
        ),
    )?;
    Ok(StartedService {
        state,
        displaced,
    })
}
