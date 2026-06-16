//! # Text Converter Plugin
//!
//! This plugin provides text file encoding conversion functionality.
//! It supports converting between various text encodings including UTF-8,
//! EUC-KR, CP949, and other encodings supported by the encoding_rs crate.

use encoding_rs::Encoding;
use plugin_interface::*;
use std::{error::Error,
          fs,
          path::{Path,
                 PathBuf}};

/// Text converter plugin that handles text file encoding conversions.
///
/// This plugin can convert text files between different character encodings,
/// making it useful for handling legacy files or preparing files for
/// different systems and locales.
pub struct TextConverterPlugin;

impl TextConverterPlugin {
    /// Creates a new instance of the TextConverterPlugin.
    pub fn new() -> Self { Self }
}

impl Default for TextConverterPlugin {
    fn default() -> Self { Self::new() }
}

impl Plugin for TextConverterPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "Text Converter".to_string(),
            version: "0.1.0".to_string(),
            author: "File Converter Team".to_string(),
            description: "텍스트 파일 인코딩 변환 플러그인".to_string(),
        }
    }

    fn supported_input_formats(&self) -> Vec<FileFormat> {
        vec![FileFormat {
            extension: "txt".to_string(),
            mime_type: "text/plain".to_string(),
            description: "Plain Text File".to_string(),
        }]
    }

    fn supported_output_formats(&self) -> Vec<FileFormat> {
        vec![FileFormat {
            extension: "txt".to_string(),
            mime_type: "text/plain".to_string(),
            description: "Plain Text File".to_string(),
        }]
    }

    fn can_convert(&self, from: &FileFormat, to: &FileFormat) -> bool {
        // This plugin only converts between text files
        from.extension == "txt" && to.extension == "txt"
    }

    fn convert(&self, input_path: &Path, _output_format: &FileFormat, options: &ConversionOptions) -> Result<ConversionResult, Box<dyn Error>> {
        // 1. Read the input file
        let input_bytes = fs::read(input_path)?;

        // 2. Detect and decode the input encoding
        // Try to get source encoding from options, otherwise auto-detect
        let source_encoding = options
            .custom_params
            .get("source_encoding")
            .and_then(|e| Encoding::for_label(e.as_bytes()))
            .unwrap_or_else(|| detect_encoding(&input_bytes));

        let (decoded_text, _, had_errors) = source_encoding.decode(&input_bytes);

        if had_errors {
            return Err(format!("Failed to decode input file with encoding: {}", source_encoding.name()).into());
        }

        // 3. Get target encoding from options (default to UTF-8)
        let target_encoding = options
            .custom_params
            .get("target_encoding")
            .and_then(|e| Encoding::for_label(e.as_bytes()))
            .unwrap_or(encoding_rs::UTF_8);

        // 4. Encode to target encoding
        let (encoded_bytes, _, had_errors) = target_encoding.encode(&decoded_text);

        if had_errors {
            return Err(format!("Failed to encode to target encoding: {}", target_encoding.name()).into());
        }

        // 5. Determine output path
        let output_path = if let Some(path) = &options.output_path {
            PathBuf::from(path)
        } else {
            generate_output_path(input_path, target_encoding.name())?
        };

        // 6. Check if output file exists and handle overwrite option
        if output_path.exists() && !options.overwrite {
            return Err(format!("Output file already exists: {}. Use overwrite option to replace it.", output_path.display()).into());
        }

        // 7. Write the output file
        fs::write(&output_path, &encoded_bytes)?;

        // 8. Return conversion result
        Ok(ConversionResult {
            success: true,
            output_path: Some(output_path.to_string_lossy().to_string()),
            message: format!("Successfully converted from {} to {}", source_encoding.name(), target_encoding.name()),
            bytes_processed: encoded_bytes.len(),
        })
    }
}

/// Detects the encoding of the input bytes.
///
/// This function attempts to detect the encoding by trying common encodings
/// in order of likelihood. It prioritizes UTF-8, then tries Korean encodings
/// (EUC-KR), and falls back to UTF-8 if detection fails.
///
/// # Arguments
///
/// * `bytes` - The input bytes to analyze
///
/// # Returns
///
/// The detected encoding, or UTF-8 as a fallback
fn detect_encoding(bytes: &[u8]) -> &'static Encoding {
    // Try UTF-8 first
    if std::str::from_utf8(bytes).is_ok() {
        return encoding_rs::UTF_8;
    }

    // Try common Korean encodings
    let korean_encodings = [encoding_rs::EUC_KR];

    for encoding in &korean_encodings {
        let (_, _, had_errors) = encoding.decode(bytes);
        if !had_errors {
            return encoding;
        }
    }

    // Default to UTF-8 if detection fails
    encoding_rs::UTF_8
}

/// Generates an output file path based on the input path and target encoding.
///
/// The output file will be in the same directory as the input file, with
/// the encoding name appended to the filename before the extension.
///
/// # Arguments
///
/// * `input_path` - The path to the input file
/// * `encoding_name` - The name of the target encoding
///
/// # Returns
///
/// A `Result` containing the generated output path
///
/// # Example
///
/// Input: `/path/to/file.txt`, encoding: `UTF-8`
/// Output: `/path/to/file_utf-8.txt`
fn generate_output_path(input_path: &Path, encoding_name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let file_stem = input_path
        .file_stem()
        .ok_or("Invalid input file path")?
        .to_str()
        .ok_or("Invalid UTF-8 in file name")?;

    let extension = input_path.extension().and_then(|e| e.to_str()).unwrap_or("txt");

    let parent = input_path.parent().ok_or("Invalid input file path")?;

    let encoding_suffix = encoding_name.to_lowercase().replace(' ', "-");
    let output_filename = format!("{}_{}.{}", file_stem, encoding_suffix, extension);

    Ok(parent.join(output_filename))
}

/// Plugin constructor function for dynamic loading.
///
/// This function is exported with C linkage to allow the plugin to be
/// dynamically loaded by the core system.
#[unsafe(no_mangle)]
pub fn create_plugin() -> Box<dyn Plugin> { Box::new(TextConverterPlugin::new()) }
