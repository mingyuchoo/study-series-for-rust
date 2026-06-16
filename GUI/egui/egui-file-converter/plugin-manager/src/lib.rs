//! Core system for file conversion
//!
//! This crate provides the core functionality for the file converter
//! application, including plugin registry management and conversion engine.

pub mod engine;
pub mod error;
pub mod loader;
pub mod registry;

// Re-export commonly used types
pub use engine::ConversionEngine;
pub use error::{ConversionError,
                ConversionResult};
pub use loader::PluginLoader;
pub use registry::PluginRegistry;
