use async_trait::async_trait;
use chrono::{DateTime,
             Utc};
use domain::{Contact,
             ContactRepository,
             DomainError};
use sqlx::{Row,
           SqlitePool,
           sqlite::SqliteRow};
use uuid::Uuid;

pub struct SqliteContactRepository {
    pool: SqlitePool,
}

/// Converts a `contacts` table row into a domain `Contact`.
///
/// Centralizes the row decoding shared by `get_by_id`, `get_all`, and `search`
/// so the column list and timestamp parsing live in a single place.
fn row_to_contact(row: &SqliteRow) -> Result<Contact, DomainError> {
    Ok(Contact {
        id: Uuid::parse_str(row.get("id")).map_err(|e| DomainError::DatabaseError(e.to_string()))?,
        name: row.get("name"),
        email: row.get("email"),
        phone: row.get("phone"),
        address: row.get("address"),
        created_at: DateTime::parse_from_rfc3339(row.get("created_at"))
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?
            .with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(row.get("updated_at"))
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?
            .with_timezone(&Utc),
    })
}

/// Escapes LIKE wildcards (`%`, `_`) and the escape char itself so that a search
/// query is matched literally. Paired with an `ESCAPE '\'` clause in the query.
fn escape_like_pattern(query: &str) -> String {
    let mut escaped = String::with_capacity(query.len());
    for ch in query.chars() {
        match ch {
            | '\\' | '%' | '_' => {
                escaped.push('\\');
                escaped.push(ch);
            },
            | _ => escaped.push(ch),
        }
    }
    escaped
}

impl SqliteContactRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
        }
    }

    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS contacts (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT,
                phone TEXT,
                address TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[async_trait]
impl ContactRepository for SqliteContactRepository {
    async fn create(&self, contact: Contact) -> Result<Contact, DomainError> {
        sqlx::query(
            r#"
            INSERT INTO contacts (id, name, email, phone, address, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(contact.id.to_string())
        .bind(&contact.name)
        .bind(&contact.email)
        .bind(&contact.phone)
        .bind(&contact.address)
        .bind(contact.created_at.to_rfc3339())
        .bind(contact.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(contact)
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<Contact>, DomainError> {
        let row = sqlx::query("SELECT id, name, email, phone, address, created_at, updated_at FROM contacts WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        match row {
            | Some(row) => Ok(Some(row_to_contact(&row)?)),
            | None => Ok(None),
        }
    }

    async fn get_all(&self) -> Result<Vec<Contact>, DomainError> {
        let rows = sqlx::query("SELECT id, name, email, phone, address, created_at, updated_at FROM contacts ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        rows.iter().map(row_to_contact).collect()
    }

    async fn update(&self, contact: Contact) -> Result<Contact, DomainError> {
        sqlx::query(
            r#"
            UPDATE contacts 
            SET name = ?, email = ?, phone = ?, address = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&contact.name)
        .bind(&contact.email)
        .bind(&contact.phone)
        .bind(&contact.address)
        .bind(contact.updated_at.to_rfc3339())
        .bind(contact.id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(contact)
    }

    async fn delete(&self, id: Uuid) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM contacts WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn search(&self, query: &str) -> Result<Vec<Contact>, DomainError> {
        let search_pattern = format!("%{}%", escape_like_pattern(query));
        let rows = sqlx::query(
            r#"
            SELECT id, name, email, phone, address, created_at, updated_at
            FROM contacts
            WHERE name LIKE ?1 ESCAPE '\'
               OR email LIKE ?1 ESCAPE '\'
               OR phone LIKE ?1 ESCAPE '\'
               OR address LIKE ?1 ESCAPE '\'
            ORDER BY name
            "#,
        )
        .bind(&search_pattern)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        rows.iter().map(row_to_contact).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ContactRepository;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn repository() -> SqliteContactRepository {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool should be created");
        let repository = SqliteContactRepository::new(pool);
        repository.init().await.expect("contacts table should be created");
        repository
    }

    #[tokio::test]
    async fn creates_lists_searches_updates_and_deletes_contacts() {
        let repository = repository().await;
        let mut contact = Contact::new(
            "  Ada Lovelace  ".to_string(),
            Some("ada@example.com".to_string()),
            Some("+82 10-1234-5678".to_string()),
            Some("Seoul".to_string()),
        );
        let id = contact.id;

        repository.create(contact.clone()).await.expect("contact should be created");

        let contacts = repository.get_all().await.expect("contacts should be listed");
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].name, "Ada Lovelace");

        let matches = repository.search("example").await.expect("contact should be searchable");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, id);

        contact.update(None, Some("".to_string()), Some("010-0000-0000".to_string()), None);
        repository.update(contact).await.expect("contact should be updated");

        let updated = repository
            .get_by_id(id)
            .await
            .expect("contact lookup should succeed")
            .expect("contact should exist");
        assert_eq!(updated.email, None);
        assert_eq!(updated.phone, Some("010-0000-0000".to_string()));

        repository.delete(id).await.expect("contact should be deleted");
        let deleted = repository.get_by_id(id).await.expect("contact lookup should succeed");
        assert_eq!(deleted, None);
    }

    #[tokio::test]
    async fn search_treats_like_wildcards_literally() {
        let repository = repository().await;
        repository
            .create(Contact::new("Ada".to_string(), Some("ada@example.com".to_string()), None, None))
            .await
            .expect("contact should be created");
        repository
            .create(Contact::new("100% Cotton".to_string(), None, None, None))
            .await
            .expect("contact should be created");

        // A bare "%" must not match every row — only the literal "100% Cotton".
        let matches = repository.search("%").await.expect("search should succeed");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "100% Cotton");
    }
}
