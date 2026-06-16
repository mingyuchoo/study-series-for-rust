// infrastructure/repositories.rs - 저장소 구현

use domain::{models::User,
             repositories::UserRepository};
use rusqlite::{Connection,
               params};
use std::sync::{Arc,
                Mutex};

// SQLite 저장소 구현 (추가)
pub struct UserDbRepository {
    conn: Arc<Mutex<Connection>>,
}

impl UserDbRepository {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let conn = Connection::open(db_path).map_err(|e| format!("Failed to open database: {}", e))?;

        // 테이블 생성
        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL,
                email TEXT NOT NULL,
                active INTEGER NOT NULL
            )",
            [],
        )
        .map_err(|e| format!("Failed to create table: {}", e))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    // SQLite 연결 테스트
    pub fn test_connection(&self) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT 1", [], |_row| Ok(()))
            .map_err(|e| format!("Database connection test failed: {}", e))
    }
}

impl UserRepository for UserDbRepository {
    fn find_by_id(&self, id: &str) -> Option<User> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, username, email, active FROM users WHERE id = ?").ok()?;
        let user_result = stmt.query_row(params![id], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                email: row.get(2)?,
                active: row.get::<_, i32>(3)? == 1,
            })
        });
        match user_result {
            | Ok(user) => Some(user),
            | Err(rusqlite::Error::QueryReturnedNoRows) => None,
            | Err(_) => None,
        }
    }

    fn save(&self, user: &User) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();

        if let Some(id) = user.id {
            conn.execute(
                "INSERT OR REPLACE INTO users (id, username, email, active) VALUES (?, ?, ?, ?)",
                params![id, user.username, user.email, if user.active { 1 } else { 0 }],
            )
            .map_err(|e| format!("Failed to save user: {}", e))?;
        } else {
            conn.execute(
                "INSERT INTO users (username, email, active) VALUES (?, ?, ?)",
                params![user.username, user.email, if user.active { 1 } else { 0 }],
            )
            .map_err(|e| format!("Failed to save user: {}", e))?;
        }
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();

        conn.execute("DELETE FROM users WHERE id = ?", params![id])
            .map_err(|e| format!("Failed to delete user: {}", e))?;

        Ok(())
    }

    fn find_all(&self) -> Vec<User> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare("SELECT id, username, email, active FROM users") {
            | Ok(stmt) => stmt,
            | Err(_) => return Vec::new(),
        };
        let users_iter = match stmt.query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                email: row.get(2)?,
                active: row.get::<_, i32>(3)? == 1,
            })
        }) {
            | Ok(iter) => iter,
            | Err(_) => return Vec::new(),
        };
        let mut users = Vec::new();
        for u in users_iter.flatten() {
            users.push(u);
        }
        users
    }
}
