use plugin_interface::{FileFormat,
                       Plugin,
                       PluginMetadata};
use std::{collections::HashMap,
          sync::{Arc,
                 RwLock}};

/// Plugin Registry manages all registered plugins
/// Provides thread-safe access to plugins using Arc<RwLock>
pub struct PluginRegistry {
    plugins: Arc<RwLock<HashMap<String, Arc<dyn Plugin>>>>,
}

impl PluginRegistry {
    /// Create a new empty plugin registry
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new plugin
    /// Returns an error if a plugin with the same name is already registered
    pub fn register_plugin(&self, plugin: Box<dyn Plugin>) -> Result<(), String> {
        let metadata = plugin.metadata();
        let mut plugins = self.plugins.write().unwrap();

        if plugins.contains_key(&metadata.name) {
            return Err(format!("Plugin '{}' already registered", metadata.name));
        }

        plugins.insert(metadata.name.clone(), Arc::from(plugin));
        Ok(())
    }

    /// Get a plugin by name
    /// Returns None if the plugin is not found
    pub fn get_plugin(&self, name: &str) -> Option<Arc<dyn Plugin>> {
        let plugins = self.plugins.read().unwrap();
        plugins.get(name).cloned()
    }

    /// List all registered plugins' metadata
    pub fn list_plugins(&self) -> Vec<PluginMetadata> {
        let plugins = self.plugins.read().unwrap();
        plugins.values().map(|p| p.metadata()).collect()
    }

    /// Find plugins that can convert from one format to another
    /// Returns a list of plugin names that support the conversion
    pub fn find_plugins_for_conversion(&self, from: &FileFormat, to: &FileFormat) -> Vec<String> {
        let plugins = self.plugins.read().unwrap();
        plugins.iter().filter(|(_, p)| p.can_convert(from, to)).map(|(name, _)| name.clone()).collect()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_interface::{ConversionOptions,
                           ConversionResult};
    use std::{error::Error,
              path::Path};

    struct MockPlugin {
        name: String,
    }

    impl MockPlugin {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    impl Plugin for MockPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata {
                name: self.name.clone(),
                version: "0.1.0".to_string(),
                author: "Test".to_string(),
                description: "Test plugin".to_string(),
            }
        }

        fn supported_input_formats(&self) -> Vec<FileFormat> {
            vec![FileFormat {
                extension: "txt".to_string(),
                mime_type: "text/plain".to_string(),
                description: "Text".to_string(),
            }]
        }

        fn supported_output_formats(&self) -> Vec<FileFormat> { self.supported_input_formats() }

        fn can_convert(&self, from: &FileFormat, to: &FileFormat) -> bool { from.extension == "txt" && to.extension == "txt" }

        fn convert(&self, _input_path: &Path, _output_format: &FileFormat, _options: &ConversionOptions) -> Result<ConversionResult, Box<dyn Error>> {
            Ok(ConversionResult {
                success: true,
                output_path: None,
                message: "Test".to_string(),
                bytes_processed: 0,
            })
        }
    }

    #[test]
    fn test_register_plugin() {
        let registry = PluginRegistry::new();
        let plugin = Box::new(MockPlugin::new("test-plugin"));

        assert!(registry.register_plugin(plugin).is_ok());
    }

    #[test]
    fn test_duplicate_plugin_registration() {
        let registry = PluginRegistry::new();
        let plugin1 = Box::new(MockPlugin::new("test-plugin"));
        let plugin2 = Box::new(MockPlugin::new("test-plugin"));

        assert!(registry.register_plugin(plugin1).is_ok());
        assert!(registry.register_plugin(plugin2).is_err());
    }

    #[test]
    fn test_get_plugin() {
        let registry = PluginRegistry::new();
        let plugin = Box::new(MockPlugin::new("test-plugin"));

        registry.register_plugin(plugin).unwrap();

        let retrieved = registry.get_plugin("test-plugin");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().metadata().name, "test-plugin");
    }

    #[test]
    fn test_list_plugins() {
        let registry = PluginRegistry::new();
        registry.register_plugin(Box::new(MockPlugin::new("plugin1"))).unwrap();
        registry.register_plugin(Box::new(MockPlugin::new("plugin2"))).unwrap();

        let plugins = registry.list_plugins();
        assert_eq!(plugins.len(), 2);
    }

    #[test]
    fn test_find_plugins_for_conversion() {
        let registry = PluginRegistry::new();
        registry.register_plugin(Box::new(MockPlugin::new("text-converter"))).unwrap();

        let from = FileFormat {
            extension: "txt".to_string(),
            mime_type: "text/plain".to_string(),
            description: "Text".to_string(),
        };
        let to = from.clone();

        let plugins = registry.find_plugins_for_conversion(&from, &to);
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0], "text-converter");
    }
}
