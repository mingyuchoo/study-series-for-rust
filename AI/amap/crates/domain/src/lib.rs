//! Canonical domain model of the Autonomous Modernization Assurance Platform (AMAP).
//!
//! Every other crate speaks these types. They are deliberately plain data:
//! the platform treats *evidence* — not an LLM's memory — as the source of truth,
//! so everything here must be serialisable, diffable and storable.

pub mod ids;
pub mod rules;
pub mod behavior;
pub mod verification;
pub mod evidence;
pub mod agents;
pub mod gate;

pub use agents::*;
pub use behavior::*;
pub use evidence::*;
pub use gate::*;
pub use ids::*;
pub use rules::*;
pub use verification::*;

/// Errors shared by domain-level validation.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("invalid identifier `{0}`")]
    InvalidId(String),
    #[error("invalid value: {0}")]
    Invalid(String),
}
