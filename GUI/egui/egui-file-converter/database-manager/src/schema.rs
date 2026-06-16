use rusqlite::{Connection,
               Result};

/// 데이터베이스 스키마 초기화
pub fn initialize_database(conn: &Connection) -> Result<()> {
    // 변환 이력 테이블
    conn.execute(
        "CREATE TABLE IF NOT EXISTS conversion_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            input_file TEXT NOT NULL,
            output_file TEXT,
            input_format TEXT NOT NULL,
            output_format TEXT NOT NULL,
            plugin_name TEXT NOT NULL,
            status TEXT NOT NULL,
            error_message TEXT,
            bytes_processed INTEGER,
            duration_ms INTEGER
        )",
        [],
    )?;

    // 사용자 설정 테이블
    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;

    // 인덱스 생성
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_history_timestamp 
         ON conversion_history(timestamp DESC)",
        [],
    )?;

    Ok(())
}
