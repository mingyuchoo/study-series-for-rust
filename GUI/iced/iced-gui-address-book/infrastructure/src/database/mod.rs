use domain::{entities::Address,
             repositories::AddressRepository};
use rusqlite::{Connection,
               Result as SqlResult};
use std::sync::Mutex;

pub struct SqliteAddressRepository {
    conn: Mutex<Connection>,
}

impl SqliteAddressRepository {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let conn = Connection::open(db_path).map_err(|e| e.to_string())?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS addresses (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                phone TEXT NOT NULL,
                email TEXT NOT NULL,
                address TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| e.to_string())?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

impl AddressRepository for SqliteAddressRepository {
    fn create(&self, mut address: Address) -> Result<Address, String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO addresses (name, phone, email, address) VALUES (?1, ?2, ?3, ?4)",
            (&address.name, &address.phone, &address.email, &address.address),
        )
        .map_err(|e| e.to_string())?;

        address.id = Some(conn.last_insert_rowid());
        Ok(address)
    }

    fn read(&self, id: i64) -> Result<Option<Address>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, name, phone, email, address FROM addresses WHERE id = ?1")
            .map_err(|e| e.to_string())?;

        let result = stmt.query_row([id], |row| {
            Ok(Address {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                phone: row.get(2)?,
                email: row.get(3)?,
                address: row.get(4)?,
            })
        });

        match result {
            | Ok(addr) => Ok(Some(addr)),
            | Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            | Err(e) => Err(e.to_string()),
        }
    }

    fn read_all(&self) -> Result<Vec<Address>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, name, phone, email, address FROM addresses")
            .map_err(|e| e.to_string())?;

        let addresses = stmt
            .query_map([], |row| {
                Ok(Address {
                    id: Some(row.get(0)?),
                    name: row.get(1)?,
                    phone: row.get(2)?,
                    email: row.get(3)?,
                    address: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<SqlResult<Vec<_>>>()
            .map_err(|e| e.to_string())?;

        Ok(addresses)
    }

    fn update(&self, address: Address) -> Result<Address, String> {
        let id = address.id.ok_or("Address must have an ID to update")?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE addresses SET name = ?1, phone = ?2, email = ?3, address = ?4 WHERE id = ?5",
            (&address.name, &address.phone, &address.email, &address.address, id),
        )
        .map_err(|e| e.to_string())?;

        Ok(address)
    }

    fn delete(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM addresses WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
        Ok(())
    }
}
