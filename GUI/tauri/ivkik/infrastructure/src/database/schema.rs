use sqlx::SqlitePool;

pub async fn init(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("PRAGMA foreign_keys = ON").execute(pool).await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS ivkik_items (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            parent_id TEXT REFERENCES ivkik_items(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            description TEXT,
            target_value REAL,
            current_value REAL,
            unit TEXT,
            position INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL,
            aggregation TEXT NOT NULL DEFAULT 'latest',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // aggregation 컬럼이 없던 기존 데이터베이스를 위한 마이그레이션.
    let has_aggregation = sqlx::query("SELECT 1 FROM pragma_table_info('ivkik_items') WHERE name = 'aggregation'")
        .fetch_optional(pool)
        .await?
        .is_some();
    if !has_aggregation {
        sqlx::query("ALTER TABLE ivkik_items ADD COLUMN aggregation TEXT NOT NULL DEFAULT 'latest'")
            .execute(pool)
            .await?;
    }

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS kpi_measurements (
            id TEXT PRIMARY KEY,
            kpi_id TEXT NOT NULL REFERENCES ivkik_items(id) ON DELETE CASCADE,
            value REAL NOT NULL,
            measured_at TEXT NOT NULL,
            note TEXT
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS item_revisions (
            id TEXT PRIMARY KEY,
            item_id TEXT NOT NULL REFERENCES ivkik_items(id) ON DELETE CASCADE,
            field TEXT NOT NULL,
            old_value TEXT,
            new_value TEXT,
            changed_at TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ivkik_items_parent ON ivkik_items(parent_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ivkik_items_kind ON ivkik_items(kind)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_kpi_measurements_kpi ON kpi_measurements(kpi_id, measured_at)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_item_revisions_item ON item_revisions(item_id, changed_at)")
        .execute(pool)
        .await?;

    Ok(())
}
