//! Shared bootstrap for the CLI and the control plane: platform settings, run specs,
//! store / lake / bus / LLM wiring, and fixture-driven mock providers.

pub mod bootstrap;
pub mod runspec;
pub mod settings;

pub use bootstrap::{Platform, PlatformBuilder};
pub use runspec::RunSpec;
pub use settings::Settings;
