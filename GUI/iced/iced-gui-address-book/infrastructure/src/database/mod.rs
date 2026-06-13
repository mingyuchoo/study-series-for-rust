use domain::{entities::Address,
             error::RepositoryError,
             repositories::AddressRepository};
use rusqlite::{Connection,
               Row};
use std::sync::{Mutex,
                MutexGuard};

/// SQLite 백엔드로 [`AddressRepository`] 를 구현한 저장소.
pub struct SqliteAddressRepository {
    conn: Mutex<Connection>,
}

/// 임의의 에러를 도메인 [`RepositoryError::Backend`] 로 변환한다.
///
/// rusqlite 에러와 뮤텍스 포이즌 에러 모두 이 함수를 거쳐 도메인 경계 밖으로
/// 백엔드 세부 타입을 노출하지 않는다.
fn backend<E: std::fmt::Display>(error: E) -> RepositoryError { RepositoryError::Backend(error.to_string()) }

/// SELECT 결과 한 행을 [`Address`] 로 매핑한다.
///
/// 이미 저장된(신뢰된) 데이터이므로 검증 없이 struct 리터럴로 재구성한다.
fn row_to_address(row: &Row<'_>) -> rusqlite::Result<Address> {
    Ok(Address {
        id: Some(row.get(0)?),
        name: row.get(1)?,
        phone: row.get(2)?,
        email: row.get(3)?,
        address: row.get(4)?,
    })
}

const SELECT_COLUMNS: &str = "SELECT id, name, phone, email, address FROM addresses";

impl SqliteAddressRepository {
    /// 파일 기반 SQLite 데이터베이스를 열고 스키마를 초기화한다.
    ///
    /// # Errors
    /// 연결 또는 테이블 생성 실패 시 [`RepositoryError::Backend`].
    pub fn new(db_path: &str) -> Result<Self, RepositoryError> {
        let conn = Connection::open(db_path).map_err(backend)?;
        Self::init(conn)
    }

    /// 인메모리 SQLite 데이터베이스로 저장소를 만든다(주로 테스트용).
    ///
    /// # Errors
    /// 연결 또는 테이블 생성 실패 시 [`RepositoryError::Backend`].
    pub fn in_memory() -> Result<Self, RepositoryError> {
        let conn = Connection::open_in_memory().map_err(backend)?;
        Self::init(conn)
    }

    /// 스키마를 생성하고 저장소를 구성한다. `new`/`in_memory` 가 공유한다.
    fn init(conn: Connection) -> Result<Self, RepositoryError> {
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
        .map_err(backend)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 포이즌을 [`RepositoryError::Backend`] 로 변환하여 연결 잠금을 획득한다.
    ///
    /// 이전 구현의 `.lock().unwrap()` 패닉을 제거한다.
    fn conn(&self) -> Result<MutexGuard<'_, Connection>, RepositoryError> { self.conn.lock().map_err(backend) }
}

impl AddressRepository for SqliteAddressRepository {
    fn create(&self, mut address: Address) -> Result<Address, RepositoryError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO addresses (name, phone, email, address) VALUES (?1, ?2, ?3, ?4)",
            (&address.name, &address.phone, &address.email, &address.address),
        )
        .map_err(backend)?;

        address.id = Some(conn.last_insert_rowid());
        Ok(address)
    }

    fn read(&self, id: i64) -> Result<Option<Address>, RepositoryError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!("{SELECT_COLUMNS} WHERE id = ?1")).map_err(backend)?;

        match stmt.query_row([id], row_to_address) {
            | Ok(addr) => Ok(Some(addr)),
            | Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            | Err(e) => Err(backend(e)),
        }
    }

    fn read_all(&self) -> Result<Vec<Address>, RepositoryError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(SELECT_COLUMNS).map_err(backend)?;

        let addresses = stmt
            .query_map([], row_to_address)
            .map_err(backend)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(backend)?;

        Ok(addresses)
    }

    fn update(&self, address: Address) -> Result<Address, RepositoryError> {
        let id = address.id.ok_or(RepositoryError::MissingId)?;
        let conn = self.conn()?;
        conn.execute(
            "UPDATE addresses SET name = ?1, phone = ?2, email = ?3, address = ?4 WHERE id = ?5",
            (&address.name, &address.phone, &address.email, &address.address, id),
        )
        .map_err(backend)?;

        Ok(address)
    }

    fn delete(&self, id: i64) -> Result<(), RepositoryError> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM addresses WHERE id = ?1", [id]).map_err(backend)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str) -> Address { Address::new(name.into(), "010-0000-0000".into(), "user@example.com".into(), "Seoul".into()).unwrap() }

    #[test]
    fn create_assigns_id_and_reads_back() {
        let repo = SqliteAddressRepository::in_memory().unwrap();
        let created = repo.create(sample("Alice")).unwrap();
        let id = created.id.expect("created address has an id");

        let fetched = repo.read(id).unwrap().expect("address should exist");
        assert_eq!(fetched.name, "Alice");
    }

    #[test]
    fn read_missing_returns_none() {
        let repo = SqliteAddressRepository::in_memory().unwrap();
        assert_eq!(repo.read(999).unwrap(), None);
    }

    #[test]
    fn read_all_returns_every_row() {
        let repo = SqliteAddressRepository::in_memory().unwrap();
        repo.create(sample("Alice")).unwrap();
        repo.create(sample("Bob")).unwrap();
        assert_eq!(repo.read_all().unwrap().len(), 2);
    }

    #[test]
    fn update_changes_fields() {
        let repo = SqliteAddressRepository::in_memory().unwrap();
        let mut created = repo.create(sample("Alice")).unwrap();
        created.name = "Alicia".into();
        repo.update(created.clone()).unwrap();

        let fetched = repo.read(created.id.unwrap()).unwrap().unwrap();
        assert_eq!(fetched.name, "Alicia");
    }

    #[test]
    fn update_without_id_is_missing_id_error() {
        let repo = SqliteAddressRepository::in_memory().unwrap();
        let orphan = sample("NoId");
        assert_eq!(repo.update(orphan), Err(RepositoryError::MissingId));
    }

    #[test]
    fn delete_removes_row() {
        let repo = SqliteAddressRepository::in_memory().unwrap();
        let created = repo.create(sample("Alice")).unwrap();
        let id = created.id.unwrap();
        repo.delete(id).unwrap();
        assert_eq!(repo.read(id).unwrap(), None);
    }
}
