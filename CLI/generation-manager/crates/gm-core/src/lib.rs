//! Domain types shared across the workspace.
//!
//! This crate owns domain types, validation and pure selection policies.
//! It performs no I/O and knows no storage paths or serialization formats.

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
