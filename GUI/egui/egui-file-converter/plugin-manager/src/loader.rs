//! Plugin loading and management
//!
//! This module provides functionality for dynamically loading plugins from
//! shared libraries. It supports scanning plugin directories, loading plugins,
//! and registering them with the plugin registry.
//!
//! # Example
//!
//! ```no_run
//! use plugin_manager::{PluginLoader,
//!                      PluginRegistry};
//!
//! // Create a plugin registry
//! let registry = PluginRegistry::new();
//!
//! // Create a plugin loader for a specific directory
//! let mut loader = PluginLoader::new("./plugins");
//!
//! // Load all plugins from the directory
//! let results = loader.load_all_plugins(&registry);
//!
//! // Check the results
//! for info in results {
//!     if info.success {
//!         println!("Successfully loaded: {} v{}", info.name, info.version);
//!     } else {
//!         eprintln!(
//!             "Failed to load {:?}: {}",
//!             info.file_path,
//!             info.error_message.unwrap_or_default()
//!         );
//!     }
//! }
//!
//! // List all registered plugins
//! let plugins = registry.list_plugins();
//! println!("Total plugins registered: {}", plugins.len());
//! ```
//!
//! # Error Handling
//!
//! The plugin loader implements graceful error handling:
//! - If a plugin fails to load, the error is logged and the loader continues
//!   with other plugins
//! - If the plugin directory doesn't exist, an empty result is returned
//! - The application continues to run even if all plugins fail to load
//!
//! # Platform Support
//!
//! The loader supports the following dynamic library formats:
//! - `.so` files on Linux
//! - `.dylib` files on macOS
//! - `.dll` files on Windows

use crate::{error::{ConversionError,
                    ConversionResult},
            registry::PluginRegistry};
use libloading::{Library,
                 Symbol};
use plugin_interface::{Plugin,
                       PluginConstructor};
use std::{fs,
          path::{Path,
                 PathBuf}};

/// Plugin loader handles dynamic loading of plugins from shared libraries
pub struct PluginLoader {
    plugin_dir: PathBuf,
    loaded_libraries: Vec<Library>,
}

/// Information about a loaded plugin
#[derive(Debug)]
pub struct LoadedPluginInfo {
    pub name: String,
    pub version: String,
    pub file_path: PathBuf,
    pub success: bool,
    pub error_message: Option<String>,
}

impl PluginLoader {
    /// Create a new plugin loader for the specified directory
    pub fn new<P: AsRef<Path>>(plugin_dir: P) -> Self {
        Self {
            plugin_dir: plugin_dir.as_ref().to_path_buf(),
            loaded_libraries: Vec::new(),
        }
    }

    /// Scan the plugin directory for plugin files
    /// Returns a list of paths to potential plugin files
    pub fn scan_plugin_directory(&self) -> ConversionResult<Vec<PathBuf>> {
        log::info!("Scanning plugin directory: {:?}", self.plugin_dir);

        if !self.plugin_dir.exists() {
            log::warn!("Plugin directory does not exist: {:?}", self.plugin_dir);
            return Ok(Vec::new());
        }

        if !self.plugin_dir.is_dir() {
            return Err(ConversionError::InvalidInput(format!("Plugin path is not a directory: {:?}", self.plugin_dir)));
        }

        let mut plugin_files = Vec::new();

        let entries = fs::read_dir(&self.plugin_dir).map_err(ConversionError::FileReadError)?;

        for entry in entries {
            let entry = entry.map_err(ConversionError::FileReadError)?;
            let path = entry.path();

            // Check if the file is a dynamic library
            if self.is_plugin_file(&path) {
                log::debug!("Found potential plugin file: {:?}", path);
                plugin_files.push(path);
            }
        }

        log::info!("Found {} potential plugin files", plugin_files.len());
        Ok(plugin_files)
    }

    /// Check if a file is a potential plugin file based on extension
    fn is_plugin_file(&self, path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }

        let extension = path.extension().and_then(|e| e.to_str());

        match extension {
            // Unix/Linux shared libraries
            | Some("so") => true,
            // macOS dynamic libraries
            | Some("dylib") => true,
            // Windows DLLs
            | Some("dll") => true,
            | _ => false,
        }
    }

    /// Get metadata from a plugin file without fully loading it
    /// This is a placeholder for future implementation
    pub fn read_plugin_metadata(&self, _plugin_path: &Path) -> ConversionResult<PluginMetadata> {
        // For now, we need to load the plugin to get metadata
        // In a more advanced implementation, we could read metadata from a separate
        // file or from a specific section of the binary
        Err(ConversionError::InvalidInput("Metadata reading not yet implemented".to_string()))
    }

    /// Load a single plugin from a file
    /// Returns the loaded plugin instance or an error
    pub fn load_plugin(&mut self, plugin_path: &Path) -> ConversionResult<Box<dyn Plugin>> {
        log::info!("Loading plugin from: {:?}", plugin_path);

        // Load the dynamic library
        let library =
            unsafe { Library::new(plugin_path).map_err(|e| ConversionError::PluginInitError(format!("Failed to load library {:?}: {}", plugin_path, e)))? };

        // Get the plugin constructor function
        let constructor: Symbol<PluginConstructor> = unsafe {
            library
                .get(b"create_plugin")
                .map_err(|e| ConversionError::PluginInitError(format!("Failed to find create_plugin function in {:?}: {}", plugin_path, e)))?
        };

        // Call the constructor to create the plugin instance
        let mut plugin = constructor();

        // Initialize the plugin
        plugin
            .initialize()
            .map_err(|e| ConversionError::PluginInitError(format!("Plugin initialization failed for {:?}: {}", plugin_path, e)))?;

        log::info!("Successfully loaded plugin: {}", plugin.metadata().name);

        // Store the library to keep it loaded
        self.loaded_libraries.push(library);

        Ok(plugin)
    }

    /// Load all plugins from the plugin directory and register them
    ///
    /// This method implements graceful error handling:
    /// - If the plugin directory doesn't exist or can't be read, it logs an
    ///   error and returns empty results
    /// - If a plugin fails to load, it logs the error and continues with other
    ///   plugins
    /// - If a plugin fails to register, it logs the error and continues with
    ///   other plugins
    /// - The application continues to run even if all plugins fail to load
    ///
    /// Returns a list of loaded plugin information including success/failure
    /// status
    pub fn load_all_plugins(&mut self, registry: &PluginRegistry) -> Vec<LoadedPluginInfo> {
        log::info!("Loading all plugins from directory: {:?}", self.plugin_dir);

        let mut results = Vec::new();

        // Scan for plugin files
        let plugin_files = match self.scan_plugin_directory() {
            | Ok(files) => files,
            | Err(e) => {
                log::error!("Failed to scan plugin directory: {}", e);
                // Application continues even if directory scan fails
                return results;
            },
        };

        if plugin_files.is_empty() {
            log::warn!("No plugin files found in directory: {:?}", self.plugin_dir);
            return results;
        }

        // Load each plugin - failures are logged but don't stop the process
        for plugin_path in plugin_files {
            let info = self.load_and_register_plugin(&plugin_path, registry);
            results.push(info);
        }

        let success_count = results.iter().filter(|r| r.success).count();
        let total_count = results.len();

        log::info!(
            "Plugin loading complete: {} successful, {} failed out of {} total",
            success_count,
            total_count - success_count,
            total_count
        );

        results
    }

    /// Load and register a single plugin with comprehensive error handling
    /// This is a helper method that ensures errors are properly logged and
    /// handled
    fn load_and_register_plugin(&mut self, plugin_path: &Path, registry: &PluginRegistry) -> LoadedPluginInfo {
        match self.load_plugin(plugin_path) {
            | Ok(plugin) => {
                let metadata = plugin.metadata();
                let name = metadata.name.clone();
                let version = metadata.version.clone();

                // Attempt to register the plugin
                match registry.register_plugin(plugin) {
                    | Ok(_) => {
                        log::info!("✓ Successfully loaded and registered plugin: {} v{}", name, version);
                        LoadedPluginInfo {
                            name,
                            version,
                            file_path: plugin_path.to_path_buf(),
                            success: true,
                            error_message: None,
                        }
                    },
                    | Err(e) => {
                        log::error!("✗ Failed to register plugin {}: {}", name, e);
                        LoadedPluginInfo {
                            name,
                            version,
                            file_path: plugin_path.to_path_buf(),
                            success: false,
                            error_message: Some(format!("Registration failed: {}", e)),
                        }
                    },
                }
            },
            | Err(e) => {
                log::error!("✗ Failed to load plugin from {:?}: {}", plugin_path, e);
                LoadedPluginInfo {
                    name: "Unknown".to_string(),
                    version: "Unknown".to_string(),
                    file_path: plugin_path.to_path_buf(),
                    success: false,
                    error_message: Some(format!("Load failed: {}", e)),
                }
            },
        }
    }

    /// Get statistics about loaded plugins
    pub fn get_load_statistics(results: &[LoadedPluginInfo]) -> PluginLoadStatistics {
        let total = results.len();
        let successful = results.iter().filter(|r| r.success).count();
        let failed = total - successful;

        PluginLoadStatistics {
            total,
            successful,
            failed,
        }
    }
}

/// Statistics about plugin loading
#[derive(Debug, Clone)]
pub struct PluginLoadStatistics {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
}

/// Placeholder for plugin metadata structure
/// This would be used for reading metadata without loading the full plugin
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub file_path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    #[test]
    fn test_scan_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let loader = PluginLoader::new(temp_dir.path());

        let plugins = loader.scan_plugin_directory().unwrap();
        assert_eq!(plugins.len(), 0);
    }

    #[test]
    fn test_scan_nonexistent_directory() {
        let loader = PluginLoader::new("/nonexistent/path");
        let plugins = loader.scan_plugin_directory().unwrap();
        assert_eq!(plugins.len(), 0);
    }

    #[test]
    fn test_is_plugin_file() {
        let temp_dir = TempDir::new().unwrap();
        let loader = PluginLoader::new(temp_dir.path());

        // Create test files
        let so_file = temp_dir.path().join("plugin.so");
        let dylib_file = temp_dir.path().join("plugin.dylib");
        let dll_file = temp_dir.path().join("plugin.dll");
        let txt_file = temp_dir.path().join("readme.txt");

        File::create(&so_file).unwrap();
        File::create(&dylib_file).unwrap();
        File::create(&dll_file).unwrap();
        File::create(&txt_file).unwrap();

        assert!(loader.is_plugin_file(&so_file));
        assert!(loader.is_plugin_file(&dylib_file));
        assert!(loader.is_plugin_file(&dll_file));
        assert!(!loader.is_plugin_file(&txt_file));
    }

    #[test]
    fn test_scan_directory_with_plugins() {
        let temp_dir = TempDir::new().unwrap();

        // Create some plugin files
        File::create(temp_dir.path().join("plugin1.so")).unwrap();
        File::create(temp_dir.path().join("plugin2.so")).unwrap();
        File::create(temp_dir.path().join("readme.txt")).unwrap();

        let loader = PluginLoader::new(temp_dir.path());
        let plugins = loader.scan_plugin_directory().unwrap();

        assert_eq!(plugins.len(), 2);
    }
}
