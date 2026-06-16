use chrono::{DateTime,
             Utc};
use rusqlite::{Connection,
               Result,
               params};

/// 변환 이력 항목
#[derive(Debug, Clone)]
pub struct ConversionHistoryEntry {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub input_file: String,
    pub output_file: Option<String>,
    pub input_format: String,
    pub output_format: String,
    pub plugin_name: String,
    pub status: String,
    pub error_message: Option<String>,
    pub bytes_processed: usize,
    pub duration_ms: u64,
}

/// 변환 이력 관리자
pub struct HistoryManager {
    conn: Connection,
}

impl HistoryManager {
    /// 새로운 HistoryManager 생성
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        crate::schema::initialize_database(&conn)?;
        Ok(Self {
            conn,
        })
    }

    /// 변환 이력 추가
    pub fn add_entry(&self, entry: &ConversionHistoryEntry) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO conversion_history 
             (timestamp, input_file, output_file, input_format, output_format, 
              plugin_name, status, error_message, bytes_processed, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                entry.timestamp.to_rfc3339(),
                entry.input_file,
                entry.output_file,
                entry.input_format,
                entry.output_format,
                entry.plugin_name,
                entry.status,
                entry.error_message,
                entry.bytes_processed as i64,
                entry.duration_ms as i64,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// 최근 이력 조회 (최대 100개)
    pub fn get_recent_entries(&self, limit: usize) -> Result<Vec<ConversionHistoryEntry>> {
        let limit = limit.min(100); // 최대 100개로 제한

        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, input_file, output_file, input_format, 
                    output_format, plugin_name, status, error_message, 
                    bytes_processed, duration_ms
             FROM conversion_history
             ORDER BY timestamp DESC
             LIMIT ?1",
        )?;

        let entries = stmt.query_map([limit], |row| {
            let timestamp_str: String = row.get(1)?;
            let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(ConversionHistoryEntry {
                id: row.get(0)?,
                timestamp,
                input_file: row.get(2)?,
                output_file: row.get(3)?,
                input_format: row.get(4)?,
                output_format: row.get(5)?,
                plugin_name: row.get(6)?,
                status: row.get(7)?,
                error_message: row.get(8)?,
                bytes_processed: row.get::<_, i64>(9)? as usize,
                duration_ms: row.get::<_, i64>(10)? as u64,
            })
        })?;

        entries.collect()
    }
}
