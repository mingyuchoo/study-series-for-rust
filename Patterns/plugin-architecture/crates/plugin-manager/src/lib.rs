use libloading::{Library,
                 Symbol};
use plugin_interface::{Plugin,
                       PluginContext,
                       PluginCreate};
use std::error::Error;
use std::fs;
use std::path::Path;

/// Manages the lifecycle of dynamically loaded plugins.
///
/// The `PluginManager` is responsible for:
/// - Discovering plugin libraries in a directory
/// - Loading plugins dynamically at runtime
/// - Managing plugin lifecycle (load, execute, unload)
/// - Handling errors gracefully without crashing the application
///
/// # Implementation Details
///
/// The manager stores both plugin instances and library handles to prevent
/// premature unloading of the dynamic libraries. Libraries must remain loaded
/// for the lifetime of their plugin instances.
///
/// # Examples
///
/// ```no_run
/// use std::path::PathBuf;
/// # use plugin_manager::PluginManager;
/// # use plugin_interface::PluginContext;
///
/// let mut manager = PluginManager::new();
/// let plugin_dir = PathBuf::from("target/debug/plugins");
///
/// // Discover and load plugins
/// manager.discover_plugins(&plugin_dir).unwrap();
///
/// // Execute all plugins
/// let context = PluginContext::new();
/// let results = manager.execute_all(&context);
///
/// // Cleanup
/// manager.shutdown().unwrap();
/// ```
pub struct PluginManager {
    /// Loaded plugin instances
    plugins: Vec<Box<dyn Plugin>>,
    /// Dynamic library handles (kept alive to prevent premature unloading)
    libraries: Vec<Library>,
}

impl Default for PluginManager {
    fn default() -> Self { Self::new() }
}

impl PluginManager {
    /// Creates a new `PluginManager` instance.
    ///
    /// The manager starts with no plugins loaded. Use `discover_plugins()`
    /// or `load_plugin()` to load plugins.
    ///
    /// # Examples
    ///
    /// ```
    /// # use plugin_manager::PluginManager;
    /// let manager = PluginManager::new();
    /// ```
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            libraries: Vec::new(),
        }
    }

    /// Discovers all plugin libraries in the specified directory.
    ///
    /// Scans the directory for files with platform-specific dynamic library
    /// extensions:
    /// - `.so` on Linux
    /// - `.dll` on Windows
    /// - `.dylib` on macOS
    ///
    /// For each discovered library, attempts to load it as a plugin. If a
    /// plugin fails to load, logs an error and continues with the next
    /// plugin.
    ///
    /// # Arguments
    ///
    /// * `plugin_dir` - Path to the directory containing plugin libraries
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the directory was successfully scanned, even if some
    /// plugins failed to load. Returns an error only if the directory cannot be
    /// read.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::path::PathBuf;
    /// # use plugin_manager::PluginManager;
    /// let mut manager = PluginManager::new();
    /// let plugin_dir = PathBuf::from("target/debug/plugins");
    /// manager.discover_plugins(&plugin_dir).unwrap();
    /// ```
    pub fn discover_plugins(&mut self, plugin_dir: &Path) -> Result<(), Box<dyn Error>> {
        if !plugin_dir.exists() {
            eprintln!("Plugin directory does not exist: {:?}", plugin_dir);
            return Ok(());
        }

        let entries = fs::read_dir(plugin_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_file()
                && let Some(extension) = path.extension()
            {
                let ext = extension.to_string_lossy();
                if (ext == "so" || ext == "dll" || ext == "dylib")
                    && let Err(e) = self.load_plugin(&path)
                {
                    eprintln!("Failed to load plugin {:?}: {}", path, e);
                }
            }
        }

        Ok(())
    }

    /// Loads a single plugin from the specified path.
    ///
    /// Uses `libloading` to dynamically load the library and resolve the
    /// `_plugin_create` symbol. After loading, calls the plugin's `on_load()`
    /// method for initialization.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the plugin library file
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the plugin was successfully loaded and initialized.
    /// Returns an error if:
    /// - The library cannot be loaded
    /// - The `_plugin_create` symbol is not found
    /// - The constructor returns a null pointer
    ///
    /// # Safety
    ///
    /// This function uses `unsafe` code to:
    /// - Load the dynamic library
    /// - Resolve and call the constructor function
    /// - Convert the raw pointer to a `Box<dyn Plugin>`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::path::PathBuf;
    /// # use plugin_manager::PluginManager;
    /// let mut manager = PluginManager::new();
    /// let plugin_path = PathBuf::from("target/debug/plugins/libhello_plugin.so");
    /// manager.load_plugin(&plugin_path).unwrap();
    /// ```
    pub fn load_plugin(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        unsafe {
            let library = Library::new(path)?;

            let constructor: Symbol<PluginCreate> = library.get(b"_plugin_create")?;
            let plugin_ptr = constructor();

            if plugin_ptr.is_null() {
                return Err("Plugin constructor returned null".into());
            }

            let mut plugin = Box::from_raw(plugin_ptr);

            // Call on_load
            if let Err(e) = plugin.on_load() {
                eprintln!("Plugin {} on_load failed: {}", plugin.name(), e);
            } else {
                println!("Loaded plugin: {} v{} - {}", plugin.name(), plugin.version(), plugin.description());
            }

            self.plugins.push(plugin);
            self.libraries.push(library);

            Ok(())
        }
    }

    /// Executes all loaded plugins with the provided context.
    ///
    /// Calls the `execute()` method on each loaded plugin, passing the context
    /// data. Plugins are executed in the order they were loaded.
    ///
    /// # Arguments
    ///
    /// * `context` - Runtime context data to pass to each plugin
    ///
    /// # Returns
    ///
    /// Returns a vector of results, one for each plugin. Each result contains
    /// either the plugin's output string or an error. A plugin error does not
    /// prevent other plugins from executing.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use plugin_manager::PluginManager;
    /// # use plugin_interface::PluginContext;
    /// # let manager = PluginManager::new();
    /// let mut context = PluginContext::new();
    /// context.data.insert("user".to_string(), "Alice".to_string());
    ///
    /// let results = manager.execute_all(&context);
    /// for (i, result) in results.iter().enumerate() {
    ///     match result {
    ///         | Ok(output) => println!("Plugin {}: {}", i, output),
    ///         | Err(e) => eprintln!("Plugin {} error: {}", i, e),
    ///     }
    /// }
    /// ```
    pub fn execute_all(&self, context: &PluginContext) -> Vec<Result<String, Box<dyn Error>>> {
        self.plugins.iter().map(|plugin| plugin.execute(context)).collect()
    }

    /// Shuts down all plugins in reverse order of loading.
    ///
    /// Calls `on_unload()` for each plugin to allow proper cleanup. Plugins
    /// are unloaded in reverse order to handle any potential dependencies.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` even if some plugins fail to unload cleanly. Errors
    /// are logged but do not prevent other plugins from being unloaded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use plugin_manager::PluginManager;
    /// # let mut manager = PluginManager::new();
    /// manager.shutdown().unwrap();
    /// ```
    pub fn shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        for plugin in self.plugins.iter_mut().rev() {
            if let Err(e) = plugin.on_unload() {
                eprintln!("Plugin {} on_unload failed: {}", plugin.name(), e);
            } else {
                println!("Unloaded plugin: {}", plugin.name());
            }
        }
        Ok(())
    }
}
