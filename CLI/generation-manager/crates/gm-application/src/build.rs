use crate::{ArtifactCollector,
            Clock,
            Error,
            GenerationRepository,
            Result,
            SourceControl,
            StageExecutor,
            StoredGeneration};
use gm_core::{Config,
              Generation,
              GenerationStatus};
use std::path::Path;

pub struct BuildGeneration<'a> {
    pub source: &'a Path,
    pub worktree: Option<&'a str>,
    pub note: Option<String>,
}

pub fn build_generation(
    request: BuildGeneration<'_>,
    config: &Config,
    repository: &impl GenerationRepository,
    stages: &impl StageExecutor,
    source_control: &impl SourceControl,
    artifacts: &impl ArtifactCollector,
    clock: &impl Clock,
) -> Result<StoredGeneration> {
    config.validate()?;
    if !config.build.is_empty() {
        Error::port("running build stage", stages.run_configured("build", &config.build, request.source))?;
    }
    if !config.test.is_empty() {
        Error::port("running test stage", stages.run_configured("test", &config.test, request.source))?;
    }

    let commit = source_control.head_commit(request.source);
    let dirty = source_control.is_dirty(request.source);
    let staged = Error::port("staging generation", repository.stage(commit.as_deref()))?;
    let collected = match artifacts.collect(request.source, &config.artifacts.include, &staged.payload) {
        | Ok(paths) => paths,
        | Err(source) => {
            let _ = repository.discard(staged);
            return Err(crate::Error::Port {
                operation: "collecting artifacts",
                source,
            });
        },
    };

    let generation = Generation {
        id: staged.id,
        commit,
        worktree: request.worktree.map(str::to_string),
        dirty,
        built_at: clock.now(),
        status: GenerationStatus::Built,
        artifacts: collected,
        note: request.note,
    };
    Error::port("committing generation", repository.commit(staged, generation))
}
