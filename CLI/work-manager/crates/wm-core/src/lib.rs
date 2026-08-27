//! Domain types shared across the workspace.
//!
//! This crate is deliberately I/O-free apart from (de)serialization: it owns the
//! vocabulary (`Config`, `Generation`, `GenerationId`) and nothing else.

pub mod config;
pub mod error;
pub mod generation;
pub mod run;

pub use config::{BuildStage, Config, Discovery, HealthCheck, Preset, RunStage};
pub use error::{Error, Result};
pub use generation::{Generation, GenerationId, GenerationStatus, SwitchEvent};
pub use run::{RunSource, RunState};
