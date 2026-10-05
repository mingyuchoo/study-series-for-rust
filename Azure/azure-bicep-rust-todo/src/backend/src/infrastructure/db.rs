use sqlx::SqlitePool;

pub async fn create_database_pool() -> Result<SqlitePool, sqlx::Error> {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let db_path = current_dir.join("todos.db");
    let database_url = format!("sqlite:{}?mode=rwc", db_path.display());

    println!("Current directory: {:?}", current_dir);
    println!("Connecting to database: {}", database_url);

    let pool = SqlitePool::connect(&database_url).await?;
    println!("Successfully connected to database");

    Ok(pool)
}

pub async fn initialize_database(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS todos (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT,
            completed BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    println!("Database initialized successfully");
    Ok(())
}
