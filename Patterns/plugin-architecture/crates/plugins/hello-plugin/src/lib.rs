use plugin_interface::{Plugin,
                       PluginContext};
use std::error::Error;

/// A simple plugin that generates greeting messages.
///
/// This plugin demonstrates the basic structure of a plugin implementation.
/// It shows how to:
/// - Implement the `Plugin` trait
/// - Manage plugin state (initialization flag)
/// - Access context data during execution
/// - Provide metadata (name, version, description)
///
/// # Examples
///
/// The plugin generates personalized greetings based on context data:
/// - If context contains a "name" key, uses that value
/// - Otherwise, defaults to "World"
///
/// # Implementation Notes
///
/// This plugin maintains an `initialized` flag to demonstrate state management
/// and validation. In production plugins, this could be used to track more
/// complex initialization state.
pub struct HelloPlugin {
    /// Flag indicating whether the plugin has been initialized
    initialized: bool,
}

impl Default for HelloPlugin {
    fn default() -> Self { Self::new() }
}

impl HelloPlugin {
    /// Creates a new `HelloPlugin` instance.
    ///
    /// The plugin starts in an uninitialized state. The `on_load()` method
    /// must be called to initialize the plugin before execution.
    pub fn new() -> Self {
        Self {
            initialized: false,
        }
    }
}

impl Plugin for HelloPlugin {
    fn name(&self) -> &str { "Hello Plugin" }

    fn version(&self) -> &str { "0.1.0" }

    fn description(&self) -> &str { "A simple plugin that generates personalized greeting messages" }

    fn on_load(&mut self) -> Result<(), Box<dyn Error>> {
        println!("[HelloPlugin] Initializing...");
        self.initialized = true;
        println!("[HelloPlugin] Initialization complete");
        Ok(())
    }

    fn execute(&self, context: &PluginContext) -> Result<String, Box<dyn Error>> {
        // Validate that the plugin has been initialized
        if !self.initialized {
            return Err("Plugin not initialized".into());
        }

        // Get the name from context data, or use a default
        // This demonstrates how plugins can access runtime data
        let name = context.data.get("name").map(|s| s.as_str()).unwrap_or("World");

        // Generate a personalized greeting
        let greeting = format!("Hello, {}! Welcome to the plugin system.", name);

        Ok(greeting)
    }

    fn on_unload(&mut self) -> Result<(), Box<dyn Error>> {
        println!("[HelloPlugin] Cleaning up...");
        self.initialized = false;
        println!("[HelloPlugin] Cleanup complete");
        Ok(())
    }
}

/// Plugin constructor function for dynamic loading.
///
/// This function is called by the plugin manager to create an instance of the
/// plugin. It must be exported with C ABI and no name mangling to be
/// discoverable by the dynamic loader.
///
/// # Safety
///
/// This function returns a raw pointer to a trait object. The caller (plugin
/// manager) is responsible for:
/// - Converting the pointer back to a `Box<dyn Plugin>`
/// - Managing the lifetime of the plugin instance
/// - Calling `on_unload()` before dropping the plugin
///
/// # Implementation Pattern
///
/// All plugins must follow this pattern:
/// 1. Create a new plugin instance
/// 2. Box the instance
/// 3. Convert to a raw pointer with `Box::into_raw()`
/// 4. Return the raw pointer
#[allow(improper_ctypes_definitions)]
#[unsafe(no_mangle)]
pub extern "C" fn _plugin_create() -> *mut dyn Plugin {
    let plugin = Box::new(HelloPlugin::new());
    Box::into_raw(plugin)
}
