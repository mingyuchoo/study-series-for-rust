use sqlx::SqlitePool;

pub async fn init(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("PRAGMA foreign_keys = ON").execute(pool).await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS ikik_items (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            parent_id TEXT REFERENCES ikik_items(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            description TEXT,
            target_value REAL,
            current_value REAL,
            unit TEXT,
            position INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL,
            aggregation TEXT NOT NULL DEFAULT 'latest',
            due_date TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // 이전 버전 데이터베이스에는 due_date 컬럼이 없다. 추가를 시도하고
    // "duplicate column" 오류만 무시해 멱등하게 따라잡는다.
    if let Err(error) = sqlx::query("ALTER TABLE ikik_items ADD COLUMN due_date TEXT").execute(pool).await
        && !error.to_string().contains("duplicate column")
    {
        return Err(error);
    }

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS kpi_measurements (
            id TEXT PRIMARY KEY,
            kpi_id TEXT NOT NULL REFERENCES ikik_items(id) ON DELETE CASCADE,
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
            item_id TEXT NOT NULL REFERENCES ikik_items(id) ON DELETE CASCADE,
            field TEXT NOT NULL,
            old_value TEXT,
            new_value TEXT,
            changed_at TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ikik_items_parent ON ikik_items(parent_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ikik_items_kind ON ikik_items(kind)")
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
