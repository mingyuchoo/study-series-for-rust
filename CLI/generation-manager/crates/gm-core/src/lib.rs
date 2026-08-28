//! Domain types shared across the workspace.
//!
//! This crate is deliberately I/O-free apart from (de)serialization: it owns
//! the vocabulary (`Config`, `Generation`, `GenerationId`) and nothing else.

pub mod config;
pub mod error;
pub mod generation;
pub mod policy;
pub mod run;

pub use config::{BuildStage,
                 Config,
                 DetectedFiles,
                 HealthCheck,
                 Preset,
                 RunStage};
pub use error::{Error,
                Result};
pub use generation::{Generation,
                     GenerationId,
                     GenerationStatus,
                     SwitchEvent};
pub use policy::{activation_restore_target,
                 gc_candidates,
                 next_generation_id,
                 rollback_target,
                 select_worktree};
pub use run::{RunSource,
              RunState};
