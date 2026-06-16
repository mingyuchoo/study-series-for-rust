use plugin_interface::{Plugin,
                       PluginContext};
use std::error::Error;

/// A plugin that performs mathematical calculations.
///
/// This plugin demonstrates a more complex plugin implementation with:
/// - State management (operation counter)
/// - Input validation and error handling
/// - Context data parsing
/// - Multiple operation modes
///
/// # Supported Operations
///
/// The plugin supports the following mathematical operations:
/// - `add`: Addition (a + b)
/// - `subtract`: Subtraction (a - b)
/// - `multiply`: Multiplication (a * b)
/// - `divide`: Division (a / b) with zero-check
///
/// # Context Data Requirements
///
/// The plugin expects the following keys in the context data:
/// - `operation`: The operation to perform (add, subtract, multiply, divide)
/// - `a`: First operand (must be a valid number)
/// - `b`: Second operand (must be a valid number)
///
/// # Error Handling
///
/// The plugin returns errors for:
/// - Missing required context data
/// - Invalid number format
/// - Division by zero
/// - Unknown operations
pub struct MathPlugin {
    /// Flag indicating whether the plugin has been initialized
    initialized: bool,
    /// Counter tracking the number of operations performed
    operation_count: u32,
}

impl Default for MathPlugin {
    fn default() -> Self { Self::new() }
}

impl MathPlugin {
    /// Creates a new `MathPlugin` instance.
    ///
    /// The plugin starts in an uninitialized state with an operation count of
    /// zero. The `on_load()` method must be called to initialize the plugin
    /// before execution.
    pub fn new() -> Self {
        Self {
            initialized: false,
            operation_count: 0,
        }
    }
}

impl Plugin for MathPlugin {
    fn name(&self) -> &str { "Math Plugin" }

    fn version(&self) -> &str { "0.1.0" }

    fn description(&self) -> &str { "A plugin that performs mathematical calculations based on context data" }

    fn on_load(&mut self) -> Result<(), Box<dyn Error>> {
        println!("[MathPlugin] Initializing...");
        self.initialized = true;
        self.operation_count = 0;
        println!("[MathPlugin] Initialization complete");
        Ok(())
    }

    fn execute(&self, context: &PluginContext) -> Result<String, Box<dyn Error>> {
        // Validate that the plugin has been initialized
        if !self.initialized {
            return Err("Plugin not initialized".into());
        }

        // Extract operation type from context data
        // Returns an error if the key is missing
        let operation = context.data.get("operation").ok_or("Missing 'operation' in context data")?;

        // Extract operands from context data
        // Returns errors if either operand is missing
        let a_str = context.data.get("a").ok_or("Missing operand 'a' in context data")?;
        let b_str = context.data.get("b").ok_or("Missing operand 'b' in context data")?;

        // Parse operands as floating-point numbers
        // Returns descriptive errors if parsing fails
        let a: f64 = a_str.parse().map_err(|_| format!("Invalid number for operand 'a': {}", a_str))?;
        let b: f64 = b_str.parse().map_err(|_| format!("Invalid number for operand 'b': {}", b_str))?;

        // Perform calculation based on the requested operation
        // Each operation is validated and handled appropriately
        let result = match operation.as_str() {
            | "add" => a + b,
            | "subtract" => a - b,
            | "multiply" => a * b,
            | "divide" => {
                // Validate divisor to prevent division by zero
                if b == 0.0 {
                    return Err("Division by zero".into());
                }
                a / b
            },
            // Return an error for unsupported operations
            | _ => return Err(format!("Unknown operation: {}", operation).into()),
        };

        // Format and return the result as a string
        Ok(format!("{} {} {} = {}", a, operation, b, result))
    }

    fn on_unload(&mut self) -> Result<(), Box<dyn Error>> {
        println!("[MathPlugin] Cleaning up...");
        println!("[MathPlugin] Total operations performed: {}", self.operation_count);
        self.initialized = false;
        println!("[MathPlugin] Cleanup complete");
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
    let plugin = Box::new(MathPlugin::new());
    Box::into_raw(plugin)
}
