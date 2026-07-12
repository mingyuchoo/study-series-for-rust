use async_trait::async_trait;
use domain::{entities::Address,
             repositories::AddressRepository};
use sqlx::{Row,
           SqlitePool};
use uuid::Uuid;

pub struct SqliteAddressRepository {
    pool: SqlitePool,
}

impl SqliteAddressRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
        }
    }

    pub async fn init_database(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS addresses (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                phone TEXT NOT NULL,
                email TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl AddressRepository for SqliteAddressRepository {
    async fn create(&self, address: Address) -> domain::repositories::Result<Address> {
        sqlx::query(
            r#"
            INSERT INTO addresses (id, name, phone, email)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(address.id.to_string())
        .bind(&address.name)
        .bind(&address.phone)
        .bind(&address.email)
        .execute(&self.pool)
        .await?;

        Ok(address)
    }

    async fn get_by_id(&self, id: Uuid) -> domain::repositories::Result<Option<Address>> {
        let row = sqlx::query("SELECT * FROM addresses WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            | Some(row) => {
                let address = Address {
                    id: Uuid::parse_str(&row.get::<String, _>("id"))?,
                    name: row.get("name"),
                    phone: row.get("phone"),
                    email: row.get("email"),
                };
                Ok(Some(address))
            },
            | None => Ok(None),
        }
    }

    async fn get_all(&self) -> domain::repositories::Result<Vec<Address>> {
        let rows = sqlx::query("SELECT * FROM addresses ORDER BY name").fetch_all(&self.pool).await?;

        let addresses = rows
            .into_iter()
            .map(|row| {
                Ok(Address {
                    id: Uuid::parse_str(&row.get::<String, _>("id"))?,
                    name: row.get("name"),
                    phone: row.get("phone"),
                    email: row.get("email"),
                })
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error + Send + Sync>>>()?;

        Ok(addresses)
    }

    async fn update(&self, address: Address) -> domain::repositories::Result<Address> {
        sqlx::query(
            r#"
            UPDATE addresses 
            SET name = ?, phone = ?, email = ?
            WHERE id = ?
            "#,
        )
        .bind(&address.name)
        .bind(&address.phone)
        .bind(&address.email)
        .bind(address.id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(address)
    }

    async fn delete(&self, id: Uuid) -> domain::repositories::Result<bool> {
        let result = sqlx::query("DELETE FROM addresses WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
