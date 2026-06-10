mod models;
mod routes;

use application::*;
use infrastructure::SqliteVvkikRepository;
use routes::*;
use sqlx::{SqlitePool,
           sqlite::{SqliteConnectOptions,
                    SqliteJournalMode}};
use std::{fs,
          path::Path,
          sync::Arc,
          time::Duration};
use tauri::Manager;

async fn setup_database(app_data_dir: &Path) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    fs::create_dir_all(app_data_dir)?;

    let database_path = app_data_dir.join("vvkik.sqlite");
    let options = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePool::connect_with(options).await?;

    let repository = SqliteVvkikRepository::new(pool.clone());
    repository.init().await?;

    Ok(pool)
}

fn build_app_state(pool: SqlitePool) -> AppState {
    let repository = Arc::new(SqliteVvkikRepository::new(pool));

    AppState {
        create_item_use_case: Arc::new(CreateItemUseCase::new(repository.clone())),
        list_items_use_case: Arc::new(ListItemsUseCase::new(repository.clone())),
        update_item_use_case: Arc::new(UpdateItemUseCase::new(repository.clone())),
        delete_item_use_case: Arc::new(DeleteItemUseCase::new(repository.clone())),
        search_items_use_case: Arc::new(SearchItemsUseCase::new(repository.clone())),
        record_kpi_measurement_use_case: Arc::new(RecordKpiMeasurementUseCase::new(repository)),
    }
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter,
                             fmt};

    // Honors RUST_LOG; defaults to `info` so command failures are visible.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let pool = tauri::async_runtime::block_on(setup_database(&app_data_dir))?;
            app.manage(build_app_state(pool));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_item,
            list_items,
            update_item,
            delete_item,
            search_items,
            record_kpi_measurement
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{ItemKind,
                 VvkikItem,
                 VvkikRepository};
    use uuid::Uuid;

    #[tokio::test]
    async fn setup_database_creates_a_persistent_sqlite_file() {
        let app_data_dir = std::env::temp_dir().join(format!("tauri-dioxus-vvkik-{}", Uuid::new_v4()));

        let pool = setup_database(&app_data_dir).await.expect("database should be initialized");
        assert!(app_data_dir.join("vvkik.sqlite").exists());

        let repository = SqliteVvkikRepository::new(pool.clone());
        repository
            .create_item(VvkikItem::new(domain::NewVvkikItem {
                kind: ItemKind::Value,
                parent_id: None,
                title: "Freedom".to_string(),
                description: None,
                target_value: None,
                current_value: None,
                unit: None,
                position: 0,
            }))
            .await
            .expect("item should be created");
        pool.close().await;

        let pool = setup_database(&app_data_dir).await.expect("database should reopen");
        let repository = SqliteVvkikRepository::new(pool.clone());
        let items = repository.list_items().await.expect("items should be loaded");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Freedom");

        pool.close().await;
        let _ = fs::remove_dir_all(app_data_dir);
    }
}
