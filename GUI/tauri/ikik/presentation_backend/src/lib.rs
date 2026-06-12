mod models;
mod routes;

use application::*;
use infrastructure::{SqliteIkikRepository,
                     seed_if_empty};
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

    let database_path = app_data_dir.join("ikik.sqlite");
    let options = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePool::connect_with(options).await?;

    let repository = SqliteIkikRepository::new(pool.clone());
    repository.init().await?;

    if seed_if_empty(&repository).await? {
        tracing::info!("seeded initial IKIK example data");
    }

    Ok(pool)
}

fn build_app_state(pool: SqlitePool) -> AppState {
    AppState {
        repository: Arc::new(SqliteIkikRepository::new(pool)),
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
            record_kpi_measurement,
            list_kpi_measurements,
            delete_kpi_measurement,
            list_all_kpi_measurements,
            list_item_revisions
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{IkikItem,
                 ItemKind,
                 ItemRepository};
    use uuid::Uuid;

    #[tokio::test]
    async fn setup_database_creates_a_persistent_seeded_sqlite_file() {
        let app_data_dir = std::env::temp_dir().join(format!("tauri-dioxus-ikik-{}", Uuid::new_v4()));

        let pool = setup_database(&app_data_dir).await.expect("database should be initialized");
        assert!(app_data_dir.join("ikik.sqlite").exists());

        // 첫 실행에서 Identity 2 → Key Result Area 8 → Income Generating Task 16
        // → Key Performance Indicator 32 시드(총 58개)가 들어간다.
        let repository = SqliteIkikRepository::new(pool.clone());
        let seeded_items = repository.list_items().await.expect("items should be loaded");
        assert_eq!(seeded_items.len(), 58);

        repository
            .create_item(IkikItem::new(domain::NewIkikItem {
                kind: ItemKind::Identity,
                parent_id: None,
                title: "Freedom".to_string(),
                description: None,
                target_value: None,
                current_value: None,
                unit: None,
                position: 2,
                aggregation: domain::KpiAggregation::default(),
                due_date: None,
            }))
            .await
            .expect("item should be created");
        pool.close().await;

        // 두 번째 실행은 사용자 데이터를 보존하고 시드를 다시 넣지 않는다.
        let pool = setup_database(&app_data_dir).await.expect("database should reopen");
        let repository = SqliteIkikRepository::new(pool.clone());
        let items = repository.list_items().await.expect("items should be loaded");

        assert_eq!(items.len(), 59);
        assert!(items.iter().any(|item| item.title == "Freedom"));

        pool.close().await;
        let _ = fs::remove_dir_all(app_data_dir);
    }
}
