use std::collections::HashMap;
use std::error::Error;

/// Context data passed to plugins during execution.
///
/// The `PluginContext` provides a way to pass runtime data to plugins
/// through a key-value store. This allows the core application to provide
/// configuration, user input, or other dynamic data to plugins at execution
/// time.
///
/// # Examples
///
/// ```
/// use plugin_interface::PluginContext;
///
/// let mut context = PluginContext::new();
/// context.data.insert("user".to_string(), "Alice".to_string());
/// context.data.insert("value".to_string(), "42".to_string());
/// ```
pub struct PluginContext {
    /// Key-value store for passing arbitrary data to plugins
    pub data: HashMap<String, String>,
}

impl Default for PluginContext {
    fn default() -> Self { Self::new() }
}

impl PluginContext {
    /// Creates a new empty `PluginContext`.
    ///
    /// # Examples
    ///
    /// ```
    /// use plugin_interface::PluginContext;
    ///
    /// let context = PluginContext::new();
    /// assert!(context.data.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
}

/// Plugin trait that all plugins must implement.
///
/// This trait defines the contract for all plugins in the system.
/// Plugins must be thread-safe (`Send + Sync`) to allow concurrent access
/// by the plugin manager.
///
/// # Lifecycle
///
/// Plugins follow a three-phase lifecycle:
/// 1. **Loading**: The plugin is dynamically loaded and `on_load()` is called
/// 2. **Execution**: The plugin's `execute()` method is called with context
///    data
/// 3. **Unloading**: The plugin's `on_unload()` method is called for cleanup
///
/// # Implementation Requirements
///
/// - Plugins must be compiled as dynamic libraries (`cdylib`)
/// - Plugins must export a `_plugin_create` function with C ABI
/// - All trait methods must be implemented
///
/// # Examples
///
/// ```
/// use plugin_interface::{Plugin,
///                        PluginContext};
/// use std::error::Error;
///
/// struct MyPlugin;
///
/// impl Plugin for MyPlugin {
///     fn name(&self) -> &str { "My Plugin" }
///
///     fn version(&self) -> &str { "1.0.0" }
///
///     fn description(&self) -> &str { "A sample plugin" }
///
///     fn on_load(&mut self) -> Result<(), Box<dyn Error>> {
///         println!("Plugin loaded");
///         Ok(())
///     }
///
///     fn execute(
///         &self,
///         context: &PluginContext,
///     ) -> Result<String, Box<dyn Error>> {
///         Ok("Plugin executed".to_string())
///     }
///
///     fn on_unload(&mut self) -> Result<(), Box<dyn Error>> {
///         println!("Plugin unloaded");
///         Ok(())
///     }
/// }
/// ```
pub trait Plugin: Send + Sync {
    /// Returns the name of the plugin.
    ///
    /// This should be a human-readable identifier for the plugin.
    fn name(&self) -> &str;

    /// Returns the version of the plugin.
    ///
    /// Use semantic versioning (e.g., "1.0.0") for consistency.
    fn version(&self) -> &str;

    /// Returns a description of the plugin.
    ///
    /// This should briefly explain what the plugin does.
    fn description(&self) -> &str;

    /// Called when the plugin is loaded.
    ///
    /// Use this method to perform any initialization logic such as:
    /// - Allocating resources
    /// - Setting up internal state
    /// - Validating configuration
    ///
    /// # Errors
    ///
    /// Return an error if initialization fails. The plugin manager will
    /// log the error but continue loading other plugins.
    fn on_load(&mut self) -> Result<(), Box<dyn Error>>;

    /// Execute the plugin's main functionality.
    ///
    /// This method is called by the plugin manager to perform the plugin's
    /// core operation. The plugin can access runtime data through the context.
    ///
    /// # Arguments
    ///
    /// * `context` - Runtime context data passed to the plugin
    ///
    /// # Returns
    ///
    /// A `Result` containing the plugin's output string or an error.
    ///
    /// # Errors
    ///
    /// Return an error if execution fails. The plugin manager will handle
    /// the error gracefully without crashing the application.
    fn execute(&self, context: &PluginContext) -> Result<String, Box<dyn Error>>;

    /// Called when the plugin is unloaded.
    ///
    /// Use this method to perform any cleanup logic such as:
    /// - Releasing resources
    /// - Closing connections
    /// - Saving state
    ///
    /// # Errors
    ///
    /// Return an error if cleanup fails. The plugin manager will log
    /// the error but continue unloading other plugins.
    fn on_unload(&mut self) -> Result<(), Box<dyn Error>>;
}

/// Type alias for the plugin constructor function.
///
/// This function pointer type is used for the dynamic loading of plugins.
/// Plugins must export a function with this signature using
/// `#[unsafe(no_mangle)]` and `extern "C"` to ensure the symbol is available
/// for dynamic loading.
///
/// # Safety
///
/// This function is `unsafe` because it returns a raw pointer to a trait
/// object. The caller (plugin manager) is responsible for:
/// - Converting the raw pointer back to a `Box<dyn Plugin>`
/// - Ensuring the pointer is not null
/// - Managing the lifetime of the plugin instance
///
/// # Examples
///
/// ```
/// use plugin_interface::{Plugin, PluginContext};
/// use std::error::Error;
///
/// struct MyPlugin;
///
/// impl Plugin for MyPlugin {
///     // ... trait implementation
/// #   fn name(&self) -> &str { "My Plugin" }
/// #   fn version(&self) -> &str { "1.0.0" }
/// #   fn description(&self) -> &str { "A sample plugin" }
/// #   fn on_load(&mut self) -> Result<(), Box<dyn Error>> { Ok(()) }
/// #   fn execute(&self, context: &PluginContext) -> Result<String, Box<dyn Error>> { Ok("".to_string()) }
/// #   fn on_unload(&mut self) -> Result<(), Box<dyn Error>> { Ok(()) }
/// }
///
/// #[unsafe(no_mangle)]
/// pub extern "C" fn _plugin_create() -> *mut dyn Plugin {
///     Box::into_raw(Box::new(MyPlugin))
/// }
/// ```
#[allow(improper_ctypes_definitions)]
pub type PluginCreate = extern "C" fn() -> *mut dyn Plugin;
