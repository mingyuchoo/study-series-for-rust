use plugin_interface::{ConversionOptions,
                       FileFormat,
                       Plugin,
                       PluginMetadata};
use plugin_manager::{ConversionEngine,
                     PluginRegistry};
use std::{collections::HashMap,
          fs,
          path::{Path,
                 PathBuf},
          sync::Arc};
use tempfile::TempDir;

// Mock plugin for testing
struct MockTextPlugin;

impl Plugin for MockTextPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "Mock Text Plugin".to_string(),
            version: "0.1.0".to_string(),
            author: "Test".to_string(),
            description: "Mock plugin for testing".to_string(),
        }
    }

    fn supported_input_formats(&self) -> Vec<FileFormat> {
        vec![FileFormat {
            extension: "txt".to_string(),
            mime_type: "text/plain".to_string(),
            description: "Plain Text".to_string(),
        }]
    }

    fn supported_output_formats(&self) -> Vec<FileFormat> {
        vec![FileFormat {
            extension: "txt".to_string(),
            mime_type: "text/plain".to_string(),
            description: "Plain Text".to_string(),
        }]
    }

    fn can_convert(&self, from: &FileFormat, to: &FileFormat) -> bool { from.extension == "txt" && to.extension == "txt" }

    fn convert(
        &self,
        input_path: &Path,
        _output_format: &FileFormat,
        options: &ConversionOptions,
    ) -> Result<plugin_interface::ConversionResult, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(input_path)?;
        let converted = content.to_uppercase();

        let output_path = options.output_path.clone().unwrap_or_else(|| format!("{}_converted.txt", input_path.display()));

        fs::write(&output_path, &converted)?;

        Ok(plugin_interface::ConversionResult {
            success: true,
            output_path: Some(output_path.clone()),
            message: "Conversion successful".to_string(),
            bytes_processed: converted.len(),
        })
    }
}

fn setup_test_environment() -> (ConversionEngine, TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let registry = Arc::new(PluginRegistry::new());

    // Register mock plugin
    registry.register_plugin(Box::new(MockTextPlugin)).unwrap();

    let engine = ConversionEngine::new(registry);

    // Create test input file
    let input_file = temp_dir.path().join("test_input.txt");
    fs::write(&input_file, "hello world").unwrap();

    (engine, temp_dir, input_file)
}

#[test]
fn test_end_to_end_single_file_conversion() {
    let (engine, temp_dir, input_file) = setup_test_environment();

    let output_format = FileFormat {
        extension: "txt".to_string(),
        mime_type: "text/plain".to_string(),
        description: "Plain Text".to_string(),
    };

    let output_path = temp_dir.path().join("output.txt");
    let options = ConversionOptions {
        output_path: Some(output_path.to_string_lossy().to_string()),
        overwrite: true,
        quality: None,
        custom_params: HashMap::new(),
    };

    // Execute conversion
    let result = engine.convert_file(&input_file, &output_format, "Mock Text Plugin", &options).unwrap();

    // Verify results
    assert!(result.success);
    assert!(result.output_path.is_some());
    assert_eq!(result.message, "Conversion successful");

    // Verify output file exists and has correct content
    let output_content = fs::read_to_string(output_path).unwrap();
    assert_eq!(output_content, "HELLO WORLD");
}

#[test]
fn test_end_to_end_batch_conversion() {
    let (engine, temp_dir, _) = setup_test_environment();

    // Create multiple test files
    let file1 = temp_dir.path().join("file1.txt");
    let file2 = temp_dir.path().join("file2.txt");
    let file3 = temp_dir.path().join("file3.txt");

    fs::write(&file1, "first file").unwrap();
    fs::write(&file2, "second file").unwrap();
    fs::write(&file3, "third file").unwrap();

    let output_format = FileFormat {
        extension: "txt".to_string(),
        mime_type: "text/plain".to_string(),
        description: "Plain Text".to_string(),
    };

    let options = ConversionOptions {
        output_path: None,
        overwrite: true,
        quality: None,
        custom_params: HashMap::new(),
    };

    // Execute batch conversion
    let results = engine.batch_convert(vec![&file1, &file2, &file3], &output_format, "Mock Text Plugin", &options);

    // Verify all conversions succeeded
    assert_eq!(results.len(), 3);
    for result in results {
        assert!(result.is_ok());
        let conv_result = result.unwrap();
        assert!(conv_result.success);
    }
}

#[test]
fn test_end_to_end_error_handling() {
    let (engine, temp_dir, _) = setup_test_environment();

    let nonexistent_file = temp_dir.path().join("nonexistent.txt");

    let output_format = FileFormat {
        extension: "txt".to_string(),
        mime_type: "text/plain".to_string(),
        description: "Plain Text".to_string(),
    };

    let options = ConversionOptions {
        output_path: None,
        overwrite: true,
        quality: None,
        custom_params: HashMap::new(),
    };

    // Try to convert non-existent file
    let result = engine.convert_file(&nonexistent_file, &output_format, "Mock Text Plugin", &options);

    // Should return an error
    assert!(result.is_err());
}

#[test]
fn test_end_to_end_batch_with_partial_failure() {
    let (engine, temp_dir, _) = setup_test_environment();

    // Create some valid files and reference one invalid file
    let file1 = temp_dir.path().join("valid1.txt");
    let file2 = temp_dir.path().join("nonexistent.txt"); // This doesn't exist
    let file3 = temp_dir.path().join("valid2.txt");

    fs::write(&file1, "valid file 1").unwrap();
    fs::write(&file3, "valid file 2").unwrap();

    let output_format = FileFormat {
        extension: "txt".to_string(),
        mime_type: "text/plain".to_string(),
        description: "Plain Text".to_string(),
    };

    let options = ConversionOptions {
        output_path: None,
        overwrite: true,
        quality: None,
        custom_params: HashMap::new(),
    };

    // Execute batch conversion
    let results = engine.batch_convert(vec![&file1, &file2, &file3], &output_format, "Mock Text Plugin", &options);

    // Verify results: 2 success, 1 failure
    assert_eq!(results.len(), 3);
    assert!(results[0].is_ok());
    assert!(results[1].is_err());
    assert!(results[2].is_ok());
}

#[test]
fn test_plugin_registry_integration() {
    let registry = Arc::new(PluginRegistry::new());

    // Register plugin
    registry.register_plugin(Box::new(MockTextPlugin)).unwrap();

    // List plugins
    let plugins = registry.list_plugins();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].name, "Mock Text Plugin");

    // Get plugin
    let plugin = registry.get_plugin("Mock Text Plugin");
    assert!(plugin.is_some());

    // Find plugins for conversion
    let from = FileFormat {
        extension: "txt".to_string(),
        mime_type: "text/plain".to_string(),
        description: "Plain Text".to_string(),
    };
    let to = from.clone();

    let capable_plugins = registry.find_plugins_for_conversion(&from, &to);
    assert_eq!(capable_plugins.len(), 1);
    assert_eq!(capable_plugins[0], "Mock Text Plugin");
}
