//! Process, Git, artifact and network adapters for application ports.
//! This crate has no dependency on the filesystem store or CLI.

pub mod artifacts;
pub mod clock;
pub mod error;
pub mod exec;
pub mod git;
pub mod health;
mod process;
pub mod supervisor;

pub use supervisor::{ForegroundRun,
                     ServiceStatus,
                     Supervisor};
