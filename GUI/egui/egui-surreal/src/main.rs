use anyhow::Result;
use std::sync::{Arc,
                mpsc::channel};

mod application;
mod domain;
mod infrastructure;
mod presentation;

use application::{AuthUseCases,
                  CommandService,
                  PersonUseCases,
                  QueryUseCases};
use infrastructure::SurrealRepository;
use presentation::{AppController,
                   SurrealDbApp,
                   install_korean_fonts};

fn main() -> Result<()> {
    let (command_sender, command_receiver) = channel();
    let (response_sender, response_receiver) = channel();

    // Spawn database thread
    std::thread::spawn(move || -> Result<()> {
        let rt = tokio::runtime::Runtime::new()?;

        rt.block_on(async {
            // Initialize repository
            let repository = Arc::new(SurrealRepository::new("localhost:8000").await?);

            // Initialize use cases
            let person_use_cases = Arc::new(PersonUseCases::new(repository.clone()));
            let auth_use_cases = Arc::new(AuthUseCases::new(repository.clone()));
            let query_use_cases = Arc::new(QueryUseCases::new(repository.clone()));

            // Initialize command service
            let command_service = CommandService::new(person_use_cases, auth_use_cases, query_use_cases);

            // Main command processing loop
            loop {
                if let Ok(command) = command_receiver.try_recv() {
                    match command_service.handle_command(command).await {
                        | Ok(response) => {
                            if let Err(_) = response_sender.send(response) {
                                break; // UI thread has closed
                            }
                        },
                        | Err(e) => {
                            if let Err(_) = response_sender.send(e.to_string()) {
                                break; // UI thread has closed
                            }
                        },
                    }
                }

                // Small sleep to prevent busy waiting
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            Ok(())
        })
    });

    // Run the application
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "SurrealDB App",
        native_options,
        Box::new(|cc| {
            // 한글 IME 입력/표시를 위해 CJK 폰트 등록
            install_korean_fonts(&cc.egui_ctx);

            let controller = AppController::new(command_sender, response_receiver);
            let app = SurrealDbApp::new(controller);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run application: {}", e))
}
