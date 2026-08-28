//! Everything that touches the outside world: git, build commands, artifact
//! capture, the process supervisor, and the health probe that decides whether a
//! switch sticks.

pub mod artifacts;
pub mod error;
pub mod exec;
pub mod git;
pub mod health;
pub mod pipeline;
pub mod supervisor;

pub use gm_application::Activation;
pub use pipeline::{DevOutcome,
                   Pipeline};
pub use supervisor::{ForegroundRun,
                     ServiceStatus,
                     Supervisor};
