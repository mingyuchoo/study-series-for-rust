use crate::{error::{ConversionError,
                    ConversionResult},
            registry::PluginRegistry};
use log::{error,
          info,
          warn};
use plugin_interface::{ConversionOptions,
                       ConversionResult as PluginConversionResult,
                       FileFormat};
use std::{path::Path,
          sync::Arc};

/// Conversion Engine handles file conversion operations using registered
/// plugins
pub struct ConversionEngine {
    registry: Arc<PluginRegistry>,
}

impl ConversionEngine {
    /// Create a new conversion engine with the given plugin registry
    pub fn new(registry: Arc<PluginRegistry>) -> Self {
        Self {
            registry,
        }
    }

    /// Convert a single file to the specified output format using the given
    /// plugin
    ///
    /// # Arguments
    /// * `input_path` - Path to the input file
    /// * `output_format` - Desired output format
    /// * `plugin_name` - Name of the plugin to use for conversion
    /// * `options` - Conversion options
    ///
    /// # Returns
    /// * `Ok(PluginConversionResult)` - Conversion result on success
    /// * `Err(ConversionError)` - Error if conversion fails
    pub fn convert_file(
        &self,
        input_path: &Path,
        output_format: &FileFormat,
        plugin_name: &str,
        options: &ConversionOptions,
    ) -> ConversionResult<PluginConversionResult> {
        info!(
            "Starting conversion: {:?} -> {} using plugin '{}'",
            input_path, output_format.extension, plugin_name
        );

        // 1. Validate input file exists
        if !input_path.exists() {
            error!("Input file does not exist: {:?}", input_path);
            return Err(ConversionError::InvalidInput(format!("파일이 존재하지 않습니다: {:?}", input_path)));
        }

        if !input_path.is_file() {
            error!("Input path is not a file: {:?}", input_path);
            return Err(ConversionError::InvalidInput(format!("파일이 아닙니다: {:?}", input_path)));
        }

        // 2. Get plugin from registry
        let plugin = self.registry.get_plugin(plugin_name).ok_or_else(|| {
            error!("Plugin not found: {}", plugin_name);
            ConversionError::PluginNotFound(plugin_name.to_string())
        })?;

        // 3. Detect input format from file extension
        let input_format = self.detect_format(input_path)?;

        // 4. Verify plugin can perform the conversion
        if !plugin.can_convert(&input_format, output_format) {
            warn!(
                "Plugin '{}' cannot convert {} -> {}",
                plugin_name, input_format.extension, output_format.extension
            );
            return Err(ConversionError::NoPluginAvailable(input_format.extension, output_format.extension.clone()));
        }

        // 5. Execute conversion
        info!("Executing conversion with plugin '{}'", plugin_name);
        let result = plugin.convert(input_path, output_format, options).map_err(|e| {
            error!("Conversion failed: {}", e);
            ConversionError::ConversionFailed(e.to_string())
        })?;

        if result.success {
            info!("Conversion completed successfully: {} bytes processed", result.bytes_processed);
        } else {
            warn!("Conversion completed with warnings: {}", result.message);
        }

        Ok(result)
    }

    /// Convert multiple files in batch to the specified output format
    ///
    /// # Arguments
    /// * `files` - List of input file paths
    /// * `output_format` - Desired output format
    /// * `plugin_name` - Name of the plugin to use for conversion
    /// * `options` - Conversion options
    ///
    /// # Returns
    /// Vector of results for each file conversion
    pub fn batch_convert(
        &self,
        files: Vec<&Path>,
        output_format: &FileFormat,
        plugin_name: &str,
        options: &ConversionOptions,
    ) -> Vec<ConversionResult<PluginConversionResult>> {
        info!("Starting batch conversion of {} files", files.len());

        let results: Vec<_> = files
            .iter()
            .enumerate()
            .map(|(idx, path)| {
                info!("Processing file {}/{}: {:?}", idx + 1, files.len(), path);
                let result = self.convert_file(path, output_format, plugin_name, options);

                if let Err(ref e) = result {
                    error!("Failed to convert file {:?}: {}", path, e);
                }

                result
            })
            .collect();

        let success_count = results.iter().filter(|r| r.is_ok()).count();
        info!("Batch conversion completed: {}/{} files successful", success_count, files.len());

        results
    }

    /// Detect file format from file extension
    fn detect_format(&self, path: &Path) -> ConversionResult<FileFormat> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| ConversionError::UnsupportedFormat(format!("파일 확장자를 감지할 수 없습니다: {:?}", path)))?;

        // Get all plugins and check if any supports this format
        let plugins = self.registry.list_plugins();

        for plugin_meta in plugins {
            if let Some(plugin) = self.registry.get_plugin(&plugin_meta.name) {
                for format in plugin.supported_input_formats() {
                    if format.extension == extension {
                        return Ok(format);
                    }
                }
            }
        }

        Err(ConversionError::UnsupportedFormat(format!("지원하지 않는 파일 형식: {}", extension)))
    }

    /// Get list of available output formats for a given input file
    pub fn get_available_formats(&self, input_path: &Path) -> ConversionResult<Vec<FileFormat>> {
        let input_format = self.detect_format(input_path)?;
        let mut formats = Vec::new();

        let plugins = self.registry.list_plugins();
        for plugin_meta in plugins {
            if let Some(plugin) = self.registry.get_plugin(&plugin_meta.name) {
                for output_format in plugin.supported_output_formats() {
                    if plugin.can_convert(&input_format, &output_format) && !formats.iter().any(|f: &FileFormat| f.extension == output_format.extension) {
                        formats.push(output_format);
                    }
                }
            }
        }

        Ok(formats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_interface::{Plugin,
                           PluginMetadata};
    use std::{error::Error,
              fs,
              io::Write};

    struct MockPlugin;

    impl Plugin for MockPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata {
                name: "mock-plugin".to_string(),
                version: "0.1.0".to_string(),
                author: "Test".to_string(),
                description: "Mock plugin for testing".to_string(),
            }
        }

        fn supported_input_formats(&self) -> Vec<FileFormat> {
            vec![FileFormat {
                extension: "txt".to_string(),
                mime_type: "text/plain".to_string(),
                description: "Text file".to_string(),
            }]
        }

        fn supported_output_formats(&self) -> Vec<FileFormat> {
            vec![
                FileFormat {
                    extension: "txt".to_string(),
                    mime_type: "text/plain".to_string(),
                    description: "Text file".to_string(),
                },
                FileFormat {
                    extension: "md".to_string(),
                    mime_type: "text/markdown".to_string(),
                    description: "Markdown file".to_string(),
                },
            ]
        }

        fn can_convert(&self, from: &FileFormat, to: &FileFormat) -> bool { from.extension == "txt" && (to.extension == "txt" || to.extension == "md") }

        fn convert(&self, input_path: &Path, _output_format: &FileFormat, _options: &ConversionOptions) -> Result<PluginConversionResult, Box<dyn Error>> {
            let content = fs::read(input_path)?;
            Ok(PluginConversionResult {
                success: true,
                output_path: Some("output.txt".to_string()),
                message: "Conversion successful".to_string(),
                bytes_processed: content.len(),
            })
        }
    }

    fn setup_test_env() -> (ConversionEngine, tempfile::TempDir) {
        let registry = Arc::new(PluginRegistry::new());
        registry.register_plugin(Box::new(MockPlugin)).unwrap();

        let engine = ConversionEngine::new(registry);
        let temp_dir = tempfile::tempdir().unwrap();

        (engine, temp_dir)
    }

    #[test]
    fn test_convert_file_success() {
        let (engine, temp_dir) = setup_test_env();

        let input_path = temp_dir.path().join("test.txt");
        let mut file = fs::File::create(&input_path).unwrap();
        file.write_all(b"test content").unwrap();

        let output_format = FileFormat {
            extension: "txt".to_string(),
            mime_type: "text/plain".to_string(),
            description: "Text".to_string(),
        };

        let options = ConversionOptions {
            output_path: None,
            overwrite: false,
            quality: None,
            custom_params: std::collections::HashMap::new(),
        };

        let result = engine.convert_file(&input_path, &output_format, "mock-plugin", &options);
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[test]
    fn test_convert_file_not_found() {
        let (engine, temp_dir) = setup_test_env();

        let input_path = temp_dir.path().join("nonexistent.txt");
        let output_format = FileFormat {
            extension: "txt".to_string(),
            mime_type: "text/plain".to_string(),
            description: "Text".to_string(),
        };

        let options = ConversionOptions {
            output_path: None,
            overwrite: false,
            quality: None,
            custom_params: std::collections::HashMap::new(),
        };

        let result = engine.convert_file(&input_path, &output_format, "mock-plugin", &options);
        assert!(result.is_err());
    }

    #[test]
    fn test_batch_convert() {
        let (engine, temp_dir) = setup_test_env();

        let file1 = temp_dir.path().join("test1.txt");
        let file2 = temp_dir.path().join("test2.txt");

        fs::File::create(&file1).unwrap().write_all(b"content1").unwrap();
        fs::File::create(&file2).unwrap().write_all(b"content2").unwrap();

        let output_format = FileFormat {
            extension: "txt".to_string(),
            mime_type: "text/plain".to_string(),
            description: "Text".to_string(),
        };

        let options = ConversionOptions {
            output_path: None,
            overwrite: false,
            quality: None,
            custom_params: std::collections::HashMap::new(),
        };

        let results = engine.batch_convert(vec![file1.as_path(), file2.as_path()], &output_format, "mock-plugin", &options);

        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
    }
}
