mod models;
mod routes;

use application::*;
use infrastructure::SqliteContactRepository;
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

    let database_path = app_data_dir.join("contacts.sqlite");
    let options = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePool::connect_with(options).await?;

    let repository = SqliteContactRepository::new(pool.clone());
    repository.init().await?;

    Ok(pool)
}

fn build_app_state(pool: SqlitePool) -> AppState {
    let repository = Arc::new(SqliteContactRepository::new(pool));

    AppState {
        create_contact_use_case: Arc::new(CreateContactUseCase::new(repository.clone())),
        get_contact_use_case: Arc::new(GetContactUseCase::new(repository.clone())),
        list_contacts_use_case: Arc::new(ListContactsUseCase::new(repository.clone())),
        update_contact_use_case: Arc::new(UpdateContactUseCase::new(repository.clone())),
        delete_contact_use_case: Arc::new(DeleteContactUseCase::new(repository.clone())),
        search_contacts_use_case: Arc::new(SearchContactsUseCase::new(repository)),
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
            create_contact,
            get_contact,
            list_contacts,
            update_contact,
            delete_contact,
            search_contacts
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Contact,
                 ContactRepository};
    use uuid::Uuid;

    #[tokio::test]
    async fn setup_database_creates_a_persistent_sqlite_file() {
        let app_data_dir = std::env::temp_dir().join(format!("tauri-dioxus-contacts-{}", Uuid::new_v4()));

        let pool = setup_database(&app_data_dir).await.expect("database should be initialized");
        assert!(app_data_dir.join("contacts.sqlite").exists());

        let repository = SqliteContactRepository::new(pool.clone());
        repository
            .create(Contact::new("Ada Lovelace".to_string(), Some("ada@example.com".to_string()), None, None))
            .await
            .expect("contact should be created");
        pool.close().await;

        let pool = setup_database(&app_data_dir).await.expect("database should reopen");
        let repository = SqliteContactRepository::new(pool.clone());
        let contacts = repository.get_all().await.expect("contacts should be loaded");

        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].name, "Ada Lovelace");

        pool.close().await;
        let _ = fs::remove_dir_all(app_data_dir);
    }
}
