//! # Plugin Interface
//!
//! This crate defines the standard interface that all file converter plugins
//! must implement. It provides the core traits, types, and structures needed to
//! create plugins that can convert files between different formats.
//!
//! ## Example
//!
//! ```rust,ignore
//! use plugin_interface::*;
//! use std::path::Path;
//! use std::error::Error;
//!
//! struct MyPlugin;
//!
//! impl Plugin for MyPlugin {
//!     fn metadata(&self) -> PluginMetadata {
//!         PluginMetadata {
//!             name: "My Plugin".to_string(),
//!             version: "1.0.0".to_string(),
//!             author: "Author Name".to_string(),
//!             description: "Plugin description".to_string(),
//!         }
//!     }
//!
//!     fn supported_input_formats(&self) -> Vec<FileFormat> {
//!         vec![/* ... */]
//!     }
//!
//!     fn supported_output_formats(&self) -> Vec<FileFormat> {
//!         vec![/* ... */]
//!     }
//!
//!     fn can_convert(&self, from: &FileFormat, to: &FileFormat) -> bool {
//!         // Implementation
//!         true
//!     }
//!
//!     fn convert(
//!         &self,
//!         input_path: &Path,
//!         output_format: &FileFormat,
//!         options: &ConversionOptions,
//!     ) -> Result<ConversionResult, Box<dyn Error>> {
//!         // Implementation
//!         Ok(ConversionResult {
//!             success: true,
//!             output_path: None,
//!             message: "Success".to_string(),
//!             bytes_processed: 0,
//!         })
//!     }
//! }
//! ```

use std::{collections::HashMap,
          error::Error,
          path::Path};

/// Metadata describing a plugin's identity and purpose.
///
/// This structure contains information about the plugin that can be displayed
/// to users and used for plugin management.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// The name of the plugin (e.g., "Text Converter")
    pub name: String,
    /// The version of the plugin (e.g., "1.0.0")
    pub version: String,
    /// The author or organization that created the plugin
    pub author: String,
    /// A brief description of what the plugin does
    pub description: String,
}

/// Represents a file format that can be used for input or output.
///
/// This structure defines a file format with its extension, MIME type,
/// and human-readable description.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileFormat {
    /// The file extension without the dot (e.g., "txt", "pdf")
    pub extension: String,
    /// The MIME type of the format (e.g., "text/plain", "application/pdf")
    pub mime_type: String,
    /// A human-readable description of the format
    pub description: String,
}

/// The result of a file conversion operation.
///
/// This structure contains information about the outcome of a conversion,
/// including success status, output location, and processing statistics.
#[derive(Debug)]
pub struct ConversionResult {
    /// Whether the conversion completed successfully
    pub success: bool,
    /// The path to the output file, if the conversion succeeded
    pub output_path: Option<String>,
    /// A message describing the result (success message or error details)
    pub message: String,
    /// The number of bytes processed during the conversion
    pub bytes_processed: usize,
}

/// Options that control how a file conversion is performed.
///
/// This structure allows customization of the conversion process through
/// various parameters and custom key-value pairs.
#[derive(Debug, Clone, Default)]
pub struct ConversionOptions {
    /// Optional custom output path. If None, the plugin should generate a
    /// default path.
    pub output_path: Option<String>,
    /// Whether to overwrite existing files at the output path
    pub overwrite: bool,
    /// Optional quality setting (0-100) for formats that support quality
    /// adjustment
    pub quality: Option<u8>,
    /// Custom parameters specific to the plugin or format
    pub custom_params: HashMap<String, String>,
}

/// The main trait that all file converter plugins must implement.
///
/// This trait defines the interface for plugins that can convert files between
/// different formats. Plugins must be thread-safe (Send + Sync) to allow
/// concurrent conversions.
///
/// # Required Methods
///
/// - `metadata()`: Returns information about the plugin
/// - `supported_input_formats()`: Lists formats the plugin can read
/// - `supported_output_formats()`: Lists formats the plugin can write
/// - `can_convert()`: Checks if a specific conversion is supported
/// - `convert()`: Performs the actual file conversion
///
/// # Optional Methods
///
/// - `initialize()`: Called once when the plugin is loaded
/// - `cleanup()`: Called when the plugin is unloaded
pub trait Plugin: Send + Sync {
    /// Returns metadata describing this plugin.
    ///
    /// This method should return information about the plugin's identity,
    /// version, and purpose.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn metadata(&self) -> PluginMetadata {
    ///     PluginMetadata {
    ///         name: "Text Converter".to_string(),
    ///         version: "1.0.0".to_string(),
    ///         author: "File Converter Team".to_string(),
    ///         description: "Converts text files between encodings".to_string(),
    ///     }
    /// }
    /// ```
    fn metadata(&self) -> PluginMetadata;

    /// Returns a list of file formats that this plugin can read as input.
    ///
    /// Each format should include the file extension, MIME type, and
    /// description.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn supported_input_formats(&self) -> Vec<FileFormat> {
    ///     vec![
    ///         FileFormat {
    ///             extension: "txt".to_string(),
    ///             mime_type: "text/plain".to_string(),
    ///             description: "Plain Text".to_string(),
    ///         },
    ///     ]
    /// }
    /// ```
    fn supported_input_formats(&self) -> Vec<FileFormat>;

    /// Returns a list of file formats that this plugin can write as output.
    ///
    /// Each format should include the file extension, MIME type, and
    /// description.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn supported_output_formats(&self) -> Vec<FileFormat> {
    ///     vec![
    ///         FileFormat {
    ///             extension: "pdf".to_string(),
    ///             mime_type: "application/pdf".to_string(),
    ///             description: "PDF Document".to_string(),
    ///         },
    ///     ]
    /// }
    /// ```
    fn supported_output_formats(&self) -> Vec<FileFormat>;

    /// Checks whether this plugin can convert from one format to another.
    ///
    /// This method should return `true` if the plugin supports converting
    /// from the `from` format to the `to` format.
    ///
    /// # Arguments
    ///
    /// * `from` - The input file format
    /// * `to` - The desired output file format
    ///
    /// # Returns
    ///
    /// `true` if the conversion is supported, `false` otherwise
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn can_convert(&self, from: &FileFormat, to: &FileFormat) -> bool {
    ///     from.extension == "txt" && to.extension == "pdf"
    /// }
    /// ```
    fn can_convert(&self, from: &FileFormat, to: &FileFormat) -> bool;

    /// Converts a file from one format to another.
    ///
    /// This is the main method that performs the actual file conversion.
    /// It should read the input file, perform the conversion, and write
    /// the output file according to the specified options.
    ///
    /// # Arguments
    ///
    /// * `input_path` - Path to the input file to convert
    /// * `output_format` - The desired output format
    /// * `options` - Options controlling the conversion process
    ///
    /// # Returns
    ///
    /// A `Result` containing either:
    /// - `Ok(ConversionResult)` with details about the successful conversion
    /// - `Err(Box<dyn Error>)` if the conversion failed
    ///
    /// # Errors
    ///
    /// This method should return an error if:
    /// - The input file cannot be read
    /// - The input format is not supported
    /// - The output format is not supported
    /// - The conversion process fails
    /// - The output file cannot be written
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn convert(
    ///     &self,
    ///     input_path: &Path,
    ///     output_format: &FileFormat,
    ///     options: &ConversionOptions,
    /// ) -> Result<ConversionResult, Box<dyn Error>> {
    ///     // Read input file
    ///     let data = std::fs::read(input_path)?;
    ///     
    ///     // Perform conversion
    ///     let converted = self.do_conversion(&data, output_format)?;
    ///     
    ///     // Write output file
    ///     let output_path = options.output_path
    ///         .clone()
    ///         .unwrap_or_else(|| generate_output_path(input_path, output_format));
    ///     std::fs::write(&output_path, converted)?;
    ///     
    ///     Ok(ConversionResult {
    ///         success: true,
    ///         output_path: Some(output_path),
    ///         message: "Conversion completed successfully".to_string(),
    ///         bytes_processed: converted.len(),
    ///     })
    /// }
    /// ```
    fn convert(&self, input_path: &Path, output_format: &FileFormat, options: &ConversionOptions) -> Result<ConversionResult, Box<dyn Error>>;

    /// Initializes the plugin.
    ///
    /// This method is called once when the plugin is first loaded. It can be
    /// used to perform any necessary setup, such as loading configuration,
    /// initializing resources, or validating dependencies.
    ///
    /// The default implementation does nothing and returns `Ok(())`.
    ///
    /// # Returns
    ///
    /// A `Result` indicating whether initialization succeeded
    ///
    /// # Errors
    ///
    /// Should return an error if initialization fails and the plugin
    /// cannot be used.
    fn initialize(&mut self) -> Result<(), Box<dyn Error>> { Ok(()) }

    /// Cleans up plugin resources.
    ///
    /// This method is called when the plugin is being unloaded. It can be
    /// used to release resources, close connections, or perform other
    /// cleanup operations.
    ///
    /// The default implementation does nothing and returns `Ok(())`.
    ///
    /// # Returns
    ///
    /// A `Result` indicating whether cleanup succeeded
    fn cleanup(&mut self) -> Result<(), Box<dyn Error>> { Ok(()) }
}

/// Type alias for a function that creates a new plugin instance.
///
/// This is used for dynamic plugin loading. Each plugin library should
/// export a function with this signature (typically named `create_plugin`)
/// that returns a boxed instance of the plugin.
///
/// # Example
///
/// ```rust,ignore
/// #[unsafe(no_mangle)]
/// pub fn create_plugin() -> Box<dyn Plugin> {
///     Box::new(MyPlugin::new())
/// }
/// ```
pub type PluginConstructor = fn() -> Box<dyn Plugin>;
