use tauri_plugin_opener;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let app_state = presentation_backend::create_app_state().await.expect("Failed to create app state");

        let builder = tauri::Builder::default().plugin(tauri_plugin_opener::init()).manage(app_state);

        presentation_backend::setup_tauri_app(builder)
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    });
}
