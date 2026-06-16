use chrono::Utc;
use rusqlite::{Connection,
               Result,
               params};
use std::collections::HashMap;

/// 애플리케이션 설정 관리자
pub struct SettingsManager {
    conn: Connection,
}

impl SettingsManager {
    /// 새로운 SettingsManager 생성
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        crate::schema::initialize_database(&conn)?;

        let manager = Self {
            conn,
        };
        manager.initialize_default_settings()?;

        Ok(manager)
    }

    /// 기본 설정값 초기화
    fn initialize_default_settings(&self) -> Result<()> {
        let defaults = Self::default_settings();

        for (key, value) in defaults {
            // 설정이 없는 경우에만 기본값 설정
            let exists: bool = self
                .conn
                .query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key = ?1)", params![key], |row| row.get(0))?;

            if !exists {
                self.save_setting(&key, &value)?;
            }
        }

        Ok(())
    }

    /// 기본 설정값 정의
    fn default_settings() -> HashMap<String, String> {
        let mut settings = HashMap::new();
        settings.insert("default_output_dir".to_string(), "".to_string());
        settings.insert("theme".to_string(), "System".to_string());
        settings.insert("language".to_string(), "ko".to_string());
        settings.insert("auto_save_history".to_string(), "true".to_string());
        settings.insert("max_history_entries".to_string(), "100".to_string());
        settings
    }

    /// 설정 저장
    pub fn save_setting(&self, key: &str, value: &str) -> Result<()> {
        let updated_at = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)",
            params![key, value, updated_at],
        )?;

        Ok(())
    }

    /// 설정 로드
    pub fn load_setting(&self, key: &str) -> Result<Option<String>> {
        let result = self.conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| row.get(0));

        match result {
            | Ok(value) => Ok(Some(value)),
            | Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            | Err(e) => Err(e),
        }
    }

    /// 모든 설정 로드
    pub fn load_all_settings(&self) -> Result<HashMap<String, String>> {
        let mut stmt = self.conn.prepare("SELECT key, value FROM settings")?;

        let settings = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;

        let mut result = HashMap::new();
        for setting in settings {
            let (key, value) = setting?;
            result.insert(key, value);
        }

        Ok(result)
    }

    /// 설정 삭제
    pub fn delete_setting(&self, key: &str) -> Result<()> {
        self.conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;

        Ok(())
    }
}
