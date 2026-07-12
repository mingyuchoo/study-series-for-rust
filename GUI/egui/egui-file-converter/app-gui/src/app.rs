use database_manager::{ConversionHistoryEntry,
                       HistoryManager,
                       SettingsManager};
use eframe::egui;
use plugin_interface::{ConversionOptions,
                       FileFormat,
                       PluginMetadata};
use plugin_manager::{ConversionEngine,
                     PluginRegistry};
use std::{sync::{Arc,
                 mpsc::{Receiver,
                        Sender,
                        channel}},
          thread};

/// Main application tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    Converter,
    History,
    Settings,
}

/// Conversion progress status
#[derive(Debug, Clone, PartialEq)]
pub enum ConversionStatus {
    Idle,
    InProgress { current_file: String, progress: f32 },
    Completed { success_count: usize, total_count: usize },
}

/// Messages sent to worker thread
#[derive(Debug)]
pub enum WorkerMessage {
    StartConversion {
        files: Vec<String>,
        output_format: FileFormat,
        plugin_name: String,
        options: ConversionOptions,
    },
}

/// Messages received from worker thread
#[derive(Debug, Clone)]
pub enum ProgressMessage {
    Started {
        total_files: usize,
    },
    Progress {
        current_file: String,
        file_index: usize,
        total_files: usize,
    },
    FileCompleted {
        file_path: String,
        success: bool,
        output_path: Option<String>,
        error: Option<String>,
    },
    Completed {
        success_count: usize,
        total_count: usize,
    },
}

/// Error dialog state
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorDialog {
    pub title: String,
    pub message: String,
    pub details: Option<String>,
}

/// File converter application
pub struct FileConverterApp {
    // Core system connection
    plugin_registry: Arc<PluginRegistry>,
    conversion_engine: Arc<ConversionEngine>,

    // Database connection
    history_manager: Option<Arc<HistoryManager>>,
    settings_manager: Option<Arc<SettingsManager>>,

    // Channels for async processing
    worker_tx: Option<Sender<WorkerMessage>>,
    progress_rx: Option<Receiver<ProgressMessage>>,

    // UI state - file selection
    selected_files: Vec<String>,

    // UI state - conversion settings
    available_plugins: Vec<PluginMetadata>,
    selected_plugin: Option<String>,
    selected_output_format: Option<FileFormat>,
    available_output_formats: Vec<FileFormat>,
    detected_input_format: Option<FileFormat>,

    // UI state - conversion progress
    conversion_status: ConversionStatus,

    // UI state - tab management
    active_tab: AppTab,

    // UI state - history
    history_entries: Vec<ConversionHistoryEntry>,
    selected_history_entry: Option<i64>, // Selected history entry ID

    // UI state - settings
    default_output_dir: String,
    theme: String,
    language: String,

    // UI state - error dialog
    error_dialog: Option<ErrorDialog>,
}

impl FileConverterApp {
    /// Create a new application instance
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Initialize Plugin Registry
        let plugin_registry = Arc::new(PluginRegistry::new());

        // Initialize Conversion Engine
        let conversion_engine = Arc::new(ConversionEngine::new(Arc::clone(&plugin_registry)));

        // Initialize History Manager (continue even if it fails)
        let history_manager = HistoryManager::new("file_converter.db").ok().map(Arc::new);

        if history_manager.is_none() {
            log::warn!("Failed to initialize history manager, history features will be disabled");
        }

        // Initialize Settings Manager (continue even if it fails)
        let settings_manager = SettingsManager::new("file_converter.db").ok().map(Arc::new);

        if settings_manager.is_none() {
            log::warn!("Failed to initialize settings manager, settings features will be disabled");
        }

        // Load settings
        let mut default_output_dir = String::new();
        let mut theme = "System".to_string();
        let mut language = "en".to_string();

        if let Some(ref settings_mgr) = settings_manager {
            if let Ok(Some(dir)) = settings_mgr.load_setting("default_output_dir") {
                default_output_dir = dir;
            }
            if let Ok(Some(t)) = settings_mgr.load_setting("theme") {
                theme = t;
            }
            if let Ok(Some(lang)) = settings_mgr.load_setting("language") {
                language = lang;
            }
        }

        // Set up worker thread and channels
        let (worker_tx, worker_rx) = channel::<WorkerMessage>();
        let (progress_tx, progress_rx) = channel::<ProgressMessage>();

        // Start worker thread
        let engine_clone = Arc::clone(&conversion_engine);
        thread::spawn(move || {
            Self::worker_thread(worker_rx, progress_tx, engine_clone);
        });

        Self {
            plugin_registry,
            conversion_engine,
            history_manager,
            settings_manager,
            worker_tx: Some(worker_tx),
            progress_rx: Some(progress_rx),
            selected_files: Vec::new(),
            available_plugins: Vec::new(),
            selected_plugin: None,
            selected_output_format: None,
            available_output_formats: Vec::new(),
            detected_input_format: None,
            conversion_status: ConversionStatus::Idle,
            active_tab: AppTab::Converter,
            history_entries: Vec::new(),
            selected_history_entry: None,
            default_output_dir,
            theme,
            language,
            error_dialog: None,
        }
    }

    /// Register a plugin
    pub fn register_plugin(&mut self, plugin: Box<dyn plugin_interface::Plugin>) -> Result<(), String> {
        let result = self.plugin_registry.register_plugin(plugin);
        if result.is_ok() {
            self.refresh_plugins();
        }
        result
    }

    /// Refresh plugin list
    pub fn refresh_plugins(&mut self) { self.available_plugins = self.plugin_registry.list_plugins(); }

    /// Refresh history list
    pub fn refresh_history(&mut self) {
        if let Some(ref history_manager) = self.history_manager {
            if let Ok(entries) = history_manager.get_recent_entries(100) {
                self.history_entries = entries;
            }
        }
    }

    /// Worker thread function - performs conversion tasks in a separate thread
    fn worker_thread(worker_rx: Receiver<WorkerMessage>, progress_tx: Sender<ProgressMessage>, engine: Arc<ConversionEngine>) {
        log::info!("Worker thread started");

        // Worker thread waits until it receives a message
        while let Ok(message) = worker_rx.recv() {
            match message {
                | WorkerMessage::StartConversion {
                    files,
                    output_format,
                    plugin_name,
                    options,
                } => {
                    log::info!("Worker thread received conversion request for {} files", files.len());

                    let total_files = files.len();

                    // Notify conversion start
                    if progress_tx
                        .send(ProgressMessage::Started {
                            total_files,
                        })
                        .is_err()
                    {
                        log::error!("Failed to send Started message");
                        break;
                    }

                    let mut success_count = 0;

                    // Convert each file sequentially
                    for (idx, file_path) in files.iter().enumerate() {
                        let path = std::path::PathBuf::from(file_path);
                        let current_file = path.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown").to_string();

                        // Update progress status (file processing started)
                        if progress_tx
                            .send(ProgressMessage::Progress {
                                current_file: current_file.clone(),
                                file_index: idx,
                                total_files,
                            })
                            .is_err()
                        {
                            log::error!("Failed to send Progress message");
                            break;
                        }

                        // Execute conversion
                        match engine.convert_file(&path, &output_format, &plugin_name, &options) {
                            | Ok(result) => {
                                let success = result.success;
                                if success {
                                    success_count += 1;
                                }

                                // Notify file completion
                                if progress_tx
                                    .send(ProgressMessage::FileCompleted {
                                        file_path: file_path.clone(),
                                        success,
                                        output_path: result.output_path,
                                        error: if success { None } else { Some(result.message) },
                                    })
                                    .is_err()
                                {
                                    log::error!("Failed to send FileCompleted message");
                                    break;
                                }

                                // Update progress after file completion (move to next file)
                                // idx + 1 represents the number of completed files
                                if progress_tx
                                    .send(ProgressMessage::Progress {
                                        current_file: if idx + 1 < total_files {
                                            "Preparing next file...".to_string()
                                        } else {
                                            "All files processed".to_string()
                                        },
                                        file_index: idx + 1,
                                        total_files,
                                    })
                                    .is_err()
                                {
                                    log::error!("Failed to send Progress update after file completion");
                                    break;
                                }
                            },
                            | Err(e) => {
                                log::error!("Failed to convert {:?}: {}", path, e);

                                // Notify file failure
                                if progress_tx
                                    .send(ProgressMessage::FileCompleted {
                                        file_path: file_path.clone(),
                                        success: false,
                                        output_path: None,
                                        error: Some(e.to_string()),
                                    })
                                    .is_err()
                                {
                                    log::error!("Failed to send FileCompleted message");
                                    break;
                                }

                                // Update progress even on failure
                                if progress_tx
                                    .send(ProgressMessage::Progress {
                                        current_file: if idx + 1 < total_files {
                                            "Preparing next file...".to_string()
                                        } else {
                                            "Processing complete".to_string()
                                        },
                                        file_index: idx + 1,
                                        total_files,
                                    })
                                    .is_err()
                                {
                                    log::error!("Failed to send Progress update after file failure");
                                    break;
                                }
                            },
                        }
                    }

                    // Notify conversion completion
                    if progress_tx
                        .send(ProgressMessage::Completed {
                            success_count,
                            total_count: total_files,
                        })
                        .is_err()
                    {
                        log::error!("Failed to send Completed message");
                        break;
                    }

                    log::info!("Worker thread completed conversion: {}/{} files successful", success_count, total_files);
                },
            }
        }

        log::info!("Worker thread terminated");
    }
}

impl eframe::App for FileConverterApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process progress messages from worker thread
        self.process_progress_messages();

        // Request UI update (when conversion is in progress)
        if matches!(self.conversion_status, ConversionStatus::InProgress { .. }) {
            ctx.request_repaint();
        }

        // Apply theme (apply every frame for consistency)
        self.apply_theme(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Show error dialog
        let mut close_dialog = false;
        if let Some(error) = &self.error_dialog {
            let error_clone = error.clone();
            egui::Window::new(&error_clone.title)
                .collapsible(false)
                .resizable(true)
                .default_width(400.0)
                .show(&ctx, |ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&error_clone.message).color(egui::Color32::RED));

                        if let Some(ref details) = error_clone.details {
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(5.0);
                            ui.label("Details:");
                            ui.add_space(5.0);

                            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                ui.label(egui::RichText::new(details).small().color(egui::Color32::GRAY));
                            });
                        }

                        ui.add_space(10.0);

                        if ui.button("OK").clicked() {
                            close_dialog = true;
                        }
                    });
                });
        }

        if close_dialog {
            self.error_dialog = None;
        }

        egui::CentralPanel::default().show(ui, |ui| {
            // Top tab menu
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, AppTab::Converter, "🔄 Converter");
                ui.selectable_value(&mut self.active_tab, AppTab::History, "📋 History");
                ui.selectable_value(&mut self.active_tab, AppTab::Settings, "⚙ Settings");
            });

            ui.separator();

            // Tab content area
            match self.active_tab {
                | AppTab::Converter => self.show_converter_tab(ui),
                | AppTab::History => self.show_history_tab(ui),
                | AppTab::Settings => self.show_settings_tab(ui),
            }
        });
    }
}

impl FileConverterApp {
    /// Show converter tab UI
    fn show_converter_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("File Conversion");

        ui.add_space(10.0);

        // File selection area
        ui.group(|ui| {
            ui.heading("📁 File Selection");

            ui.horizontal(|ui| {
                if ui.button("📄 Select File...").clicked() {
                    self.open_file_dialog(false);
                }

                if ui.button("📂 Select Multiple Files...").clicked() {
                    self.open_file_dialog(true);
                }

                if !self.selected_files.is_empty() && ui.button("🗑 Clear All").clicked() {
                    self.selected_files.clear();
                    self.selected_output_format = None;
                }
            });

            ui.add_space(5.0);

            // Display selected file list
            if self.selected_files.is_empty() {
                ui.label("No files selected.");
            } else {
                ui.label(format!("Selected files: {}", self.selected_files.len()));

                ui.add_space(5.0);

                egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                    let mut to_remove = None;

                    for (idx, file) in self.selected_files.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}.", idx + 1));
                            ui.label(file);

                            if ui.small_button("❌").clicked() {
                                to_remove = Some(idx);
                            }
                        });
                    }

                    if let Some(idx) = to_remove {
                        self.selected_files.remove(idx);
                        if self.selected_files.is_empty() {
                            self.selected_output_format = None;
                        }
                    }
                });
            }
        });

        ui.add_space(10.0);

        // Output format selection area
        if !self.selected_files.is_empty() {
            ui.group(|ui| {
                ui.heading("🎯 Output Format Selection");

                // Display current file format
                if let Some(ref input_format) = self.detected_input_format {
                    ui.horizontal(|ui| {
                        ui.label("Current format:");
                        ui.label(
                            egui::RichText::new(&input_format.extension)
                                .strong()
                                .color(egui::Color32::from_rgb(100, 150, 255)),
                        );
                        ui.label(format!("({})", input_format.description));
                    });
                } else {
                    ui.colored_label(egui::Color32::YELLOW, "⚠ Unable to detect file format.");
                }

                ui.add_space(5.0);

                // Output format dropdown
                ui.horizontal(|ui| {
                    ui.label("Convert to:");

                    let selected_text = self
                        .selected_output_format
                        .as_ref()
                        .map(|f| format!("{} ({})", f.extension, f.description))
                        .unwrap_or_else(|| "Select format".to_string());

                    let formats = self.available_output_formats.clone();
                    let mut format_changed = false;
                    let mut new_format = None;

                    egui::ComboBox::from_id_salt("output_format_combo")
                        .selected_text(selected_text)
                        .width(250.0)
                        .show_ui(ui, |ui| {
                            if formats.is_empty() {
                                ui.label("No formats available.");
                            } else {
                                for format in &formats {
                                    let label = format!("{} ({})", format.extension, format.description);
                                    let is_selected = self.selected_output_format.as_ref().map(|f| f.extension == format.extension).unwrap_or(false);

                                    if ui.selectable_label(is_selected, label).clicked() {
                                        new_format = Some(format.clone());
                                        format_changed = true;
                                    }
                                }
                            }
                        });

                    if format_changed {
                        self.selected_output_format = new_format;
                        self.update_selected_plugin();
                    }
                });

                // Display selected plugin
                if let Some(ref plugin_name) = self.selected_plugin {
                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        ui.label("Using plugin:");
                        ui.label(egui::RichText::new(plugin_name).strong().color(egui::Color32::from_rgb(100, 200, 100)));
                    });
                }
            });

            ui.add_space(10.0);
        }

        // Conversion execution button
        if !self.selected_files.is_empty() && self.selected_output_format.is_some() && self.selected_plugin.is_some() {
            ui.group(|ui| {
                ui.heading("🚀 Execute Conversion");

                let can_convert = matches!(self.conversion_status, ConversionStatus::Idle);

                ui.add_enabled_ui(can_convert, |ui| {
                    if ui.button("▶ Start Conversion").clicked() {
                        self.start_conversion();
                    }
                });

                if !can_convert {
                    ui.label("Conversion in progress...");
                }
            });

            ui.add_space(10.0);
        }

        // Display conversion progress status
        ui.group(|ui| {
            ui.heading("📊 Status");

            ui.label(format!("Available plugins: {}", self.available_plugins.len()));

            ui.add_space(5.0);

            match &self.conversion_status {
                | ConversionStatus::Idle => {
                    ui.label("⏸ Idle");
                },
                | ConversionStatus::InProgress {
                    current_file,
                    progress,
                } => {
                    ui.label(egui::RichText::new("⏳ Converting...").strong().color(egui::Color32::from_rgb(100, 150, 255)));

                    ui.add_space(5.0);

                    ui.label(format!("Current file: {}", current_file));

                    ui.add_space(5.0);

                    // Progress bar - show percentage and animation
                    let progress_bar = egui::ProgressBar::new(*progress).show_percentage().animate(true);
                    ui.add(progress_bar);

                    ui.add_space(3.0);

                    // Display progress text
                    ui.label(
                        egui::RichText::new(format!("Progress: {:.1}%", progress * 100.0))
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                },
                | ConversionStatus::Completed {
                    success_count,
                    total_count,
                } => {
                    let (icon, text, color) = if *success_count == *total_count {
                        (
                            "✅",
                            format!("Completed: {}/{} files successful", success_count, total_count),
                            egui::Color32::from_rgb(100, 200, 100),
                        )
                    } else if *success_count > 0 {
                        (
                            "⚠",
                            format!("Partially completed: {}/{} files successful", success_count, total_count),
                            egui::Color32::from_rgb(255, 200, 100),
                        )
                    } else {
                        (
                            "❌",
                            format!("Failed: 0/{} files successful", total_count),
                            egui::Color32::from_rgb(255, 100, 100),
                        )
                    };

                    ui.label(egui::RichText::new(format!("{} {}", icon, text)).strong().color(color));

                    if ui.button("🔄 Start New").clicked() {
                        self.conversion_status = ConversionStatus::Idle;
                    }
                },
            }
        });
    }

    /// Process progress messages from worker thread
    ///
    /// This function receives and processes progress status messages sent by
    /// the worker thread. Progress is calculated as completed files / total
    /// files, and updates the UI progress bar and status text.
    ///
    /// # Progress Calculation
    /// - File processing start: file_index / total_files (0%, 33%, 66%, etc.)
    /// - File processing complete: (file_index + 1) / total_files (33%, 66%,
    ///   100%, etc.)
    /// - All files complete: 100%
    fn process_progress_messages(&mut self) {
        // Collect messages first, then process (solves borrow checker issues)
        let mut messages = Vec::new();

        if let Some(ref progress_rx) = self.progress_rx {
            // Collect all pending messages
            while let Ok(message) = progress_rx.try_recv() {
                messages.push(message);
            }
        }

        // Process collected messages
        for message in messages {
            match message {
                | ProgressMessage::Started {
                    total_files,
                } => {
                    log::info!("Conversion started: {} files", total_files);
                    self.conversion_status = ConversionStatus::InProgress {
                        current_file: "Preparing...".to_string(),
                        progress: 0.0,
                    };
                },
                | ProgressMessage::Progress {
                    current_file,
                    file_index,
                    total_files,
                } => {
                    // Calculate progress: current file index / total files
                    // file_index starts from 0, so it shows the progress of the current file being
                    // processed When complete (file_index == total_files), show
                    // 100%
                    let progress = if file_index >= total_files {
                        1.0
                    } else {
                        (file_index as f32) / (total_files as f32)
                    };

                    log::debug!(
                        "Progress: {}/{} ({:.1}%) - {}",
                        file_index.min(total_files),
                        total_files,
                        progress * 100.0,
                        current_file
                    );

                    let display_index = file_index.min(total_files);
                    self.conversion_status = ConversionStatus::InProgress {
                        current_file: format!("[{}/{}] {}", display_index, total_files, current_file),
                        progress,
                    };
                },
                | ProgressMessage::FileCompleted {
                    file_path,
                    success,
                    output_path,
                    error,
                } => {
                    log::info!("File completed: {} - success: {}", file_path, success);

                    // Save to history
                    if let Some(ref output_format) = self.selected_output_format {
                        if let Some(ref plugin_name) = self.selected_plugin {
                            self.save_to_history(file_path, output_path, output_format, plugin_name, success, error);
                        }
                    }
                },
                | ProgressMessage::Completed {
                    success_count,
                    total_count,
                } => {
                    log::info!("Conversion completed: {}/{} files successful", success_count, total_count);
                    self.conversion_status = ConversionStatus::Completed {
                        success_count,
                        total_count,
                    };

                    // Show warning if some files failed
                    if success_count < total_count && success_count > 0 {
                        self.show_error(
                            "Some Files Failed",
                            &format!("{}/{} files were successfully converted.", success_count, total_count),
                            Some("Some files failed to convert. Check the History tab for details.".to_string()),
                        );
                    } else if success_count == 0 {
                        self.show_error(
                            "Conversion Failed",
                            "All files failed to convert.",
                            Some("Check the History tab for detailed error information.".to_string()),
                        );
                    }
                },
            }
        }
    }

    /// Start conversion - pass task to worker thread
    fn start_conversion(&mut self) {
        // Check if all required information is available
        let output_format = match &self.selected_output_format {
            | Some(f) => f.clone(),
            | None => {
                self.show_error(
                    "Conversion Failed",
                    "No output format selected.",
                    Some("Please select a format to convert to and try again.".to_string()),
                );
                return;
            },
        };

        let plugin_name = match &self.selected_plugin {
            | Some(p) => p.clone(),
            | None => {
                self.show_error(
                    "Conversion Failed",
                    "No plugin selected.",
                    Some("Could not find a plugin to perform the conversion.".to_string()),
                );
                return;
            },
        };

        if self.selected_files.is_empty() {
            self.show_error(
                "Conversion Failed",
                "No files selected for conversion.",
                Some("Please select files and try again.".to_string()),
            );
            return;
        }

        // Set conversion options
        let options = ConversionOptions {
            output_path: if !self.default_output_dir.is_empty() {
                Some(self.default_output_dir.clone())
            } else {
                None
            },
            overwrite: false,
            quality: None,
            custom_params: std::collections::HashMap::new(),
        };

        // Pass conversion task to worker thread
        if let Some(ref worker_tx) = self.worker_tx {
            let message = WorkerMessage::StartConversion {
                files: self.selected_files.clone(),
                output_format,
                plugin_name,
                options,
            };

            if let Err(e) = worker_tx.send(message) {
                log::error!("Failed to send conversion request to worker thread: {}", e);
                self.show_error(
                    "Conversion Failed",
                    "Unable to start conversion task.",
                    Some(format!("Worker thread communication error: {}", e)),
                );
                return;
            }

            log::info!("Conversion request sent to worker thread");

            // Set initial status
            self.conversion_status = ConversionStatus::InProgress {
                current_file: "Starting...".to_string(),
                progress: 0.0,
            };
        } else {
            self.show_error(
                "Conversion Failed",
                "Worker thread not initialized.",
                Some("Please restart the application.".to_string()),
            );
        }
    }

    /// Save conversion history
    fn save_to_history(
        &self,
        input_file: String,
        output_file: Option<String>,
        output_format: &FileFormat,
        plugin_name: &str,
        success: bool,
        error_message: Option<String>,
    ) {
        if let Some(ref history_manager) = self.history_manager {
            let entry = ConversionHistoryEntry {
                id: 0, // Will be set by database
                timestamp: chrono::Utc::now(),
                input_file,
                output_file,
                input_format: self
                    .detected_input_format
                    .as_ref()
                    .map(|f| f.extension.clone())
                    .unwrap_or_else(|| "unknown".to_string()),
                output_format: output_format.extension.clone(),
                plugin_name: plugin_name.to_string(),
                status: if success { "success".to_string() } else { "failed".to_string() },
                error_message,
                bytes_processed: 0,
                duration_ms: 0,
            };

            if let Err(e) = history_manager.add_entry(&entry) {
                log::error!("Failed to save history entry: {}", e);
            }
        }
    }

    /// Show error dialog
    fn show_error(&mut self, title: &str, message: &str, details: Option<String>) {
        self.error_dialog = Some(ErrorDialog {
            title: title.to_string(),
            message: message.to_string(),
            details,
        });
    }

    /// Open file selection dialog
    fn open_file_dialog(&mut self, multiple: bool) {
        let files = if multiple {
            // Multiple file selection
            rfd::FileDialog::new().set_title("Select Files to Convert").pick_files()
        } else {
            // Single file selection
            rfd::FileDialog::new().set_title("Select File to Convert").pick_file().map(|f| vec![f])
        };

        if let Some(paths) = files {
            for path in paths {
                let path_str = path.to_string_lossy().to_string();
                if !self.selected_files.contains(&path_str) {
                    self.selected_files.push(path_str);
                }
            }

            // When files are selected, detect input format and update output format list
            if !self.selected_files.is_empty() {
                self.detect_input_format_and_update_outputs();
            }
        }
    }

    /// Detect input file format and update available output formats
    fn detect_input_format_and_update_outputs(&mut self) {
        if self.selected_files.is_empty() {
            self.detected_input_format = None;
            self.available_output_formats.clear();
            self.selected_output_format = None;
            self.selected_plugin = None;
            return;
        }

        // Detect format based on first file
        let first_file = &self.selected_files[0];
        let path = std::path::Path::new(first_file);

        // Detect input format
        match self.conversion_engine.get_available_formats(path) {
            | Ok(formats) => {
                self.available_output_formats = formats;

                // Also detect input format
                if let Ok(input_format) = self.detect_input_format(path) {
                    self.detected_input_format = Some(input_format);
                } else {
                    self.detected_input_format = None;
                }

                // Initialize output format
                self.selected_output_format = None;
                self.selected_plugin = None;
            },
            | Err(e) => {
                log::error!("Failed to get available formats: {}", e);
                self.detected_input_format = None;
                self.available_output_formats.clear();
                self.selected_output_format = None;
                self.selected_plugin = None;

                // Show error to user
                self.show_error(
                    "Format Detection Failed",
                    "Unable to detect the format of the selected file.",
                    Some(e.to_string()),
                );
            },
        }
    }

    /// Detect input file format
    fn detect_input_format(&self, path: &std::path::Path) -> Result<FileFormat, String> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| "Unable to detect file extension.".to_string())?;

        // Check if any plugin supports this extension
        for plugin_meta in &self.available_plugins {
            if let Some(plugin) = self.plugin_registry.get_plugin(&plugin_meta.name) {
                for format in plugin.supported_input_formats() {
                    if format.extension == extension {
                        return Ok(format);
                    }
                }
            }
        }

        Err(format!("Unsupported file format: {}", extension))
    }

    /// Select plugin matching the selected output format
    fn update_selected_plugin(&mut self) {
        if let (Some(input_format), Some(output_format)) = (&self.detected_input_format, &self.selected_output_format) {
            // Find plugin that supports the conversion
            let plugins = self.plugin_registry.find_plugins_for_conversion(input_format, output_format);

            if !plugins.is_empty() {
                self.selected_plugin = Some(plugins[0].clone());
            } else {
                self.selected_plugin = None;
            }
        }
    }

    /// Show history tab UI
    fn show_history_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("📋 Conversion History");

        if self.history_manager.is_none() {
            ui.colored_label(egui::Color32::YELLOW, "⚠ Database connection failed: History features unavailable.");
            return;
        }

        ui.add_space(10.0);

        // Top control area
        ui.horizontal(|ui| {
            // History refresh button
            if ui.button("🔄 Refresh").clicked() {
                self.refresh_history();
            }

            ui.separator();

            // Display history count
            ui.label(format!("Total {} history entries", self.history_entries.len()));
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        // When history list is empty
        if self.history_entries.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.label(egui::RichText::new("📭 No conversion history.").size(16.0).color(egui::Color32::GRAY));
                ui.add_space(10.0);
                ui.label("History will appear here when you convert files.");
            });
            return;
        }

        // Split into two panels: left is list, right is details
        ui.columns(2, |columns| {
            // Left panel: History list
            columns[0].vertical(|ui| {
                ui.heading("List");
                ui.add_space(5.0);

                // Scrollable history list
                egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                    for (idx, entry) in self.history_entries.iter().enumerate() {
                        let is_selected = self.selected_history_entry == Some(entry.id);

                        // Display each history entry as a selectable group
                        let response = ui.group(|ui| {
                            ui.set_min_width(ui.available_width());

                            // Highlight selected item
                            if is_selected {
                                ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);
                            }

                            // Status icon and timestamp
                            ui.horizontal(|ui| {
                                // Status icon
                                let (icon, color) = if entry.status == "success" {
                                    ("✅", egui::Color32::from_rgb(100, 200, 100))
                                } else {
                                    ("❌", egui::Color32::from_rgb(255, 100, 100))
                                };

                                ui.label(egui::RichText::new(icon).size(14.0).color(color));

                                // Timestamp
                                let timestamp_str = entry.timestamp.format("%m-%d %H:%M").to_string();
                                ui.label(egui::RichText::new(timestamp_str).color(egui::Color32::GRAY).size(11.0));
                            });

                            ui.add_space(3.0);

                            // Input filename (shortened)
                            let input_filename = std::path::Path::new(&entry.input_file)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(&entry.input_file);

                            let short_filename = if input_filename.len() > 25 {
                                format!("{}...", &input_filename[.. 22])
                            } else {
                                input_filename.to_string()
                            };

                            ui.label(egui::RichText::new(short_filename).size(12.0));

                            // Conversion info (brief)
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&entry.input_format).small().color(egui::Color32::from_rgb(100, 150, 255)));
                                ui.label(egui::RichText::new("→").small());
                                ui.label(egui::RichText::new(&entry.output_format).small().color(egui::Color32::from_rgb(100, 200, 100)));
                            });
                        });

                        // Select on click
                        if response.response.interact(egui::Sense::click()).clicked() {
                            self.selected_history_entry = Some(entry.id);
                        }

                        // Change background color for selected item
                        if is_selected {
                            let rect = response.response.rect;
                            ui.painter().rect_filled(rect, 3.0, egui::Color32::from_rgba_premultiplied(100, 150, 255, 30));
                        }

                        // Spacing between items
                        if idx < self.history_entries.len() - 1 {
                            ui.add_space(5.0);
                        }
                    }
                });
            });

            // Right panel: Details
            columns[1].vertical(|ui| {
                ui.heading("Details");
                ui.add_space(5.0);

                // Display details if an item is selected
                if let Some(selected_id) = self.selected_history_entry {
                    if let Some(entry) = self.history_entries.iter().find(|e| e.id == selected_id) {
                        self.show_history_detail(ui, entry);
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(50.0);
                            ui.label(egui::RichText::new("Selected item not found.").color(egui::Color32::GRAY));
                        });
                    }
                } else {
                    // When no item is selected
                    ui.vertical_centered(|ui| {
                        ui.add_space(50.0);
                        ui.label(egui::RichText::new("👈 Select an item from the left").size(14.0).color(egui::Color32::GRAY));
                    });
                }
            });
        });
    }

    /// Show history detail information
    fn show_history_detail(&self, ui: &mut egui::Ui, entry: &ConversionHistoryEntry) {
        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
            // Status header
            ui.group(|ui| {
                ui.set_min_width(ui.available_width());

                let (icon, status_text, status_color) = if entry.status == "success" {
                    ("✅", "Conversion Successful", egui::Color32::from_rgb(100, 200, 100))
                } else {
                    ("❌", "Conversion Failed", egui::Color32::from_rgb(255, 100, 100))
                };

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(icon).size(20.0).color(status_color));
                    ui.label(egui::RichText::new(status_text).size(16.0).strong().color(status_color));
                });
            });

            ui.add_space(10.0);

            // Basic information
            ui.group(|ui| {
                ui.set_min_width(ui.available_width());
                ui.label(egui::RichText::new("📋 Basic Information").strong());
                ui.add_space(5.0);

                // ID
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("ID:").strong());
                    ui.label(format!("#{}", entry.id));
                });

                // Timestamp
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Time:").strong());
                    ui.label(entry.timestamp.format("%Y-%m-%d %H:%M:%S").to_string());
                });

                // Plugin
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Plugin:").strong());
                    ui.label(&entry.plugin_name);
                });
            });

            ui.add_space(10.0);

            // File information
            ui.group(|ui| {
                ui.set_min_width(ui.available_width());
                ui.label(egui::RichText::new("📁 File Information").strong());
                ui.add_space(5.0);

                // Input file
                ui.label(egui::RichText::new("Input file:").strong());
                ui.indent("input_file", |ui| {
                    ui.label(&entry.input_file);
                    ui.horizontal(|ui| {
                        ui.label("Format:");
                        ui.label(egui::RichText::new(&entry.input_format).color(egui::Color32::from_rgb(100, 150, 255)));
                    });
                });

                ui.add_space(5.0);

                // Output file
                ui.label(egui::RichText::new("Output file:").strong());
                ui.indent("output_file", |ui| {
                    if let Some(ref output_file) = entry.output_file {
                        ui.label(output_file);
                    } else {
                        ui.label(egui::RichText::new("(not created)").color(egui::Color32::GRAY).italics());
                    }
                    ui.horizontal(|ui| {
                        ui.label("Format:");
                        ui.label(egui::RichText::new(&entry.output_format).color(egui::Color32::from_rgb(100, 200, 100)));
                    });
                });
            });

            ui.add_space(10.0);

            // Processing information
            ui.group(|ui| {
                ui.set_min_width(ui.available_width());
                ui.label(egui::RichText::new("⚙ Processing Information").strong());
                ui.add_space(5.0);

                // Processed bytes
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Data processed:").strong());
                    let size_text = if entry.bytes_processed > 1024 * 1024 {
                        format!("{:.2} MB", entry.bytes_processed as f64 / (1024.0 * 1024.0))
                    } else if entry.bytes_processed > 1024 {
                        format!("{:.2} KB", entry.bytes_processed as f64 / 1024.0)
                    } else {
                        format!("{} bytes", entry.bytes_processed)
                    };
                    ui.label(size_text);
                });

                // Duration
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Duration:").strong());
                    let duration_text = if entry.duration_ms > 1000 {
                        format!("{:.2}s", entry.duration_ms as f64 / 1000.0)
                    } else {
                        format!("{}ms", entry.duration_ms)
                    };
                    ui.label(duration_text);
                });
            });

            // Error message (if failed)
            if let Some(ref error_msg) = entry.error_message {
                ui.add_space(10.0);

                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("⚠").color(egui::Color32::YELLOW));
                        ui.label(egui::RichText::new("Error Message").strong().color(egui::Color32::from_rgb(255, 150, 100)));
                    });
                    ui.add_space(5.0);

                    // Display error message in scrollable area
                    egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                        ui.label(egui::RichText::new(error_msg).color(egui::Color32::from_rgb(255, 100, 100)).monospace());
                    });
                });
            }
        });
    }

    /// Show settings tab UI
    fn show_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("⚙ Settings");

        if self.settings_manager.is_none() {
            ui.colored_label(egui::Color32::YELLOW, "⚠ Database connection failed: Settings features unavailable.");
            return;
        }

        ui.add_space(10.0);

        // Settings changed flag
        let mut settings_changed = false;

        // Default output directory setting
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.label(egui::RichText::new("📁 Default Output Directory").strong().size(14.0));
            ui.add_space(5.0);

            ui.label("Set the default directory where converted files will be saved.");
            ui.label(
                egui::RichText::new("(If empty, files will be saved in the same location as the original)")
                    .small()
                    .color(egui::Color32::GRAY),
            );

            ui.add_space(5.0);

            ui.horizontal(|ui| {
                let response = ui.text_edit_singleline(&mut self.default_output_dir);
                if response.changed() {
                    settings_changed = true;
                }

                if ui.button("📂 Browse...").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_title("Select Default Output Directory").pick_folder() {
                        self.default_output_dir = path.to_string_lossy().to_string();
                        settings_changed = true;
                    }
                }

                if !self.default_output_dir.is_empty() && ui.button("🗑 Clear").clicked() {
                    self.default_output_dir.clear();
                    settings_changed = true;
                }
            });

            // Display current setting
            if !self.default_output_dir.is_empty() {
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Current:").small());
                    ui.label(
                        egui::RichText::new(&self.default_output_dir)
                            .small()
                            .color(egui::Color32::from_rgb(100, 150, 255)),
                    );
                });
            }
        });

        ui.add_space(10.0);

        // Theme setting
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.label(egui::RichText::new("🎨 Theme").strong().size(14.0));
            ui.add_space(5.0);

            ui.label("Select the application's color theme.");

            ui.add_space(5.0);

            let old_theme = self.theme.clone();

            ui.horizontal(|ui| {
                if ui.selectable_label(self.theme == "Light", "☀ Light").clicked() {
                    self.theme = "Light".to_string();
                }
                if ui.selectable_label(self.theme == "Dark", "🌙 Dark").clicked() {
                    self.theme = "Dark".to_string();
                }
                if ui.selectable_label(self.theme == "System", "💻 System").clicked() {
                    self.theme = "System".to_string();
                }
            });

            if old_theme != self.theme {
                settings_changed = true;
                // Apply theme immediately
                self.apply_theme(ui.ctx());
            }

            ui.add_space(5.0);
            ui.label(egui::RichText::new(format!("Current theme: {}", self.theme)).small().color(egui::Color32::GRAY));
        });

        ui.add_space(10.0);

        // Language setting
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.label(egui::RichText::new("🌐 Language").strong().size(14.0));
            ui.add_space(5.0);

            ui.label("Select the application's display language.");
            ui.label(egui::RichText::new("(Currently only English is supported)").small().color(egui::Color32::GRAY));

            ui.add_space(5.0);

            let old_language = self.language.clone();

            egui::ComboBox::from_id_salt("language_combo")
                .selected_text(match self.language.as_str() {
                    | "ko" => "🇰🇷 한국어",
                    | "en" => "🇺🇸 English",
                    | "ja" => "🇯🇵 日本語",
                    | _ => "Unknown",
                })
                .width(200.0)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(self.language == "ko", "🇰🇷 한국어").clicked() {
                        self.language = "ko".to_string();
                    }
                    if ui.selectable_label(self.language == "en", "🇺🇸 English").clicked() {
                        self.language = "en".to_string();
                    }
                    if ui.selectable_label(self.language == "ja", "🇯🇵 日本語").clicked() {
                        self.language = "ja".to_string();
                    }
                });

            if old_language != self.language {
                settings_changed = true;
            }
        });

        ui.add_space(20.0);

        // Save settings button
        if settings_changed {
            ui.separator();
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("⚠ Settings have been changed.").color(egui::Color32::from_rgb(255, 200, 100)));

                if ui.button("💾 Save").clicked() {
                    self.save_settings();
                }
            });
        }

        ui.add_space(20.0);

        // Information section
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.label(egui::RichText::new("ℹ Information").strong().size(14.0));
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                ui.label("Application version:");
                ui.label(egui::RichText::new("0.1.0").strong());
            });

            ui.horizontal(|ui| {
                ui.label("Registered plugins:");
                ui.label(egui::RichText::new(format!("{}", self.available_plugins.len())).strong());
            });
        });
    }

    /// Save settings
    fn save_settings(&self) {
        if let Some(ref settings_manager) = self.settings_manager {
            // Save default output directory
            if let Err(e) = settings_manager.save_setting("default_output_dir", &self.default_output_dir) {
                log::error!("Failed to save default_output_dir: {}", e);
            }

            // Save theme
            if let Err(e) = settings_manager.save_setting("theme", &self.theme) {
                log::error!("Failed to save theme: {}", e);
            }

            // Save language
            if let Err(e) = settings_manager.save_setting("language", &self.language) {
                log::error!("Failed to save language: {}", e);
            }

            log::info!("Settings saved successfully");
        }
    }

    /// Apply theme
    fn apply_theme(&self, ctx: &egui::Context) {
        match self.theme.as_str() {
            | "Light" => {
                ctx.set_visuals(egui::Visuals::light());
            },
            | "Dark" => {
                ctx.set_visuals(egui::Visuals::dark());
            },
            | "System" => {
                // Use default for System theme
                // In reality, OS settings should be detected, but here we use Dark as default
                ctx.set_visuals(egui::Visuals::dark());
            },
            | _ => {
                ctx.set_visuals(egui::Visuals::dark());
            },
        }
    }
}
