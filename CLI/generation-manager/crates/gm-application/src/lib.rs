//! Application use cases and the narrow ports they require.
//!
//! This crate owns orchestration but no filesystem, process, network, Git, or
//! wall-clock implementation. Those capabilities are supplied by adapters.

mod activate;
mod build;
mod error;
mod ports;
mod service;
mod worktree;

pub use activate::{Activation,
                   activate_generation};
pub use build::{BuildGeneration,
                build_generation};
pub use error::{BoxError,
                Error,
                PortResult,
                Result};
pub use ports::{ArtifactCollector,
                Clock,
                DevelopmentRuntime,
                ForegroundProcess,
                GenerationRepository,
                HealthVerifier,
                RunStateRepository,
                ServiceRuntime,
                SourceControl,
                StageExecutor,
                StagedGeneration,
                StoredGeneration,
                WorktreeControl,
                WorktreeLocation,
                WorktreeRepository};
pub use service::{StartedService,
                  start_service};
pub use worktree::{RunWorktree,
                   WorktreeRun,
                   create_worktree,
                   remove_worktree,
                   run_worktree};
