//! Application composition root shared by delivery adapters.

pub mod bootstrap;
pub mod runspec;
pub mod settings;

pub use bootstrap::{Platform, PlatformBuilder};
pub use runspec::RunSpec;
pub use settings::Settings;
