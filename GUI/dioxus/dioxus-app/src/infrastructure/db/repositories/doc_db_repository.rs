// infrastructure/repositories.rs - 저장소 구현
//

use crate::domain::services::repositories::doc_repository::DocRepository;
use crate::domain::services::repositories::entities::doc::{Doc, DocForm};
use async_trait::async_trait;
#[cfg(feature = "native-db")]
use rusqlite::{Connection, params};
use std::error::Error;
#[cfg(feature = "native-db")]
use std::sync::{Arc, Mutex};

// SQLite 저장소 구현 (추가)
#[cfg(feature = "native-db")]
pub struct DocDbRepository {
    conn: Arc<Mutex<Connection>>,
}

#[cfg(not(feature = "native-db"))]
pub struct DocDbRepository {
    // Dummy implementation when native-db feature is not enabled
    _dummy: (),
}

#[cfg(feature = "native-db")]
impl DocDbRepository {
    pub fn new(db_path: &str) -> Result<Self, String> {
        // Print the database path for debugging
        println!("Opening SQLite database at: {}", db_path);

        // Ensure the directory exists
        if let Some(parent) = std::path::Path::new(db_path).parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory for database: {}", e))?;
        }

        let conn = Connection::open(db_path).map_err(|e| format!("Failed to open database: {}", e))?;

        // 테이블 생성
        conn.execute(
            "CREATE TABLE IF NOT EXISTS docs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                contents TEXT NOT NULL,
                archived INTEGER NOT NULL
            )",
            [],
        )
        .map_err(|e| format!("Failed to create table: {}", e))?;

        println!("Successfully created/opened database and table");

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

#[cfg(not(feature = "native-db"))]
impl DocDbRepository {
    pub fn new(_db_path: &str) -> Result<Self, String> {
        // Dummy implementation when native-db feature is not enabled
        Ok(Self {
            _dummy: (),
        })
    }

    // SQLite 연결 테스트 (dummy)
    pub fn test_connection(&self) -> Result<(), String> {
        // Always succeed in dummy implementation
        Ok(())
    }
}

#[cfg(feature = "native-db")]
#[async_trait(?Send)]
impl DocRepository for DocDbRepository {
    async fn fetch_all(&self) -> Result<Vec<Doc>, Box<dyn Error>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare("SELECT id, title, contents, archived FROM docs") {
            | Ok(stmt) => stmt,
            | Err(e) => return Err(Box::new(e)),
        };
        let docs_iter = match stmt.query_map([], |row| {
            Ok(Doc {
                id: row.get(0)?,
                title: row.get(1)?,
                contents: row.get(2)?,
                archived: row.get::<_, i32>(3)? == 1,
            })
        }) {
            | Ok(iter) => iter,
            | Err(e) => return Err(Box::new(e)),
        };
        let mut docs = Vec::new();
        for doc in docs_iter {
            match doc {
                | Ok(d) => docs.push(d),
                | Err(e) => return Err(Box::new(e)),
            }
        }
        Ok(docs)
    }

    async fn fetch_by_id(&self, id: i32) -> Result<Doc, Box<dyn Error>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, title, contents, archived FROM docs WHERE id = ?")?;
        let doc_result = stmt.query_row(params![id], |row| {
            Ok(Doc {
                id: row.get(0)?,
                title: row.get(1)?,
                contents: row.get(2)?,
                archived: row.get::<_, i32>(3)? == 1,
            })
        });
        match doc_result {
            | Ok(doc) => Ok(doc),
            | Err(e) => Err(Box::new(e)),
        }
    }

    async fn create(&self, doc: DocForm) -> Result<Doc, Box<dyn Error>> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO docs (title, contents, archived) VALUES (?, ?, ?)",
            params![doc.title, doc.contents, if doc.archived { 1 } else { 0 }],
        )?;

        let id = conn.last_insert_rowid() as i32;
        Ok(Doc {
            id,
            title: doc.title,
            contents: doc.contents,
            archived: doc.archived,
        })
    }

    async fn update(&self, id: i32, doc: DocForm) -> Result<Doc, Box<dyn Error>> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE docs SET title = ?, contents = ?, archived = ? WHERE id = ?",
            params![doc.title, doc.contents, if doc.archived { 1 } else { 0 }, id],
        )?;

        Ok(Doc {
            id,
            title: doc.title,
            contents: doc.contents,
            archived: doc.archived,
        })
    }

    async fn delete(&self, id: i32) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.lock().unwrap();

        conn.execute("DELETE FROM docs WHERE id = ?", params![id])?;

        Ok(())
    }
}

#[cfg(not(feature = "native-db"))]
#[async_trait(?Send)]
impl DocRepository for DocDbRepository {
    async fn fetch_all(&self) -> Result<Vec<Doc>, Box<dyn Error>> {
        // Return empty vector in dummy implementation
        Ok(Vec::new())
    }

    async fn fetch_by_id(&self, _id: i32) -> Result<Doc, Box<dyn Error>> {
        // Return error in dummy implementation
        Err("SQLite database feature is not enabled. Use the --features native-db flag".into())
    }

    async fn create(&self, _doc: DocForm) -> Result<Doc, Box<dyn Error>> {
        // Return error in dummy implementation
        Err("SQLite database feature is not enabled. Use the --features native-db flag".into())
    }

    async fn update(&self, _id: i32, _doc: DocForm) -> Result<Doc, Box<dyn Error>> {
        // Return error in dummy implementation
        Err("SQLite database feature is not enabled. Use the --features native-db flag".into())
    }

    async fn delete(&self, _id: i32) -> Result<(), Box<dyn Error>> {
        // Return error in dummy implementation
        Err("SQLite database feature is not enabled. Use the --features native-db flag".into())
    }
}
