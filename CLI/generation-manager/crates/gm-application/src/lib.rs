//! Application use cases and the narrow ports they require.
//!
//! This crate owns orchestration but no filesystem, process, network, Git, or
//! wall-clock implementation. Those capabilities are supplied by adapters.

mod activate;
mod build;
mod error;
mod ports;

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
                GenerationRepository,
                HealthVerifier,
                ServiceRuntime,
                SourceControl,
                StageExecutor,
                StagedGeneration,
                StoredGeneration};
