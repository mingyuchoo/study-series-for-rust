//! Application composition root shared by delivery adapters.

pub mod audit;
pub mod bootstrap;
pub mod runspec;
pub mod settings;

pub use audit::KnowledgeAuditSink;
pub use bootstrap::{open_knowledge, Platform, PlatformBuilder};
pub use runspec::RunSpec;
pub use settings::Settings;
