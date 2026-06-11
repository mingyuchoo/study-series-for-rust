use super::{SqliteIvkikRepository,
            mapper::row_to_item};
use async_trait::async_trait;
use domain::{DomainError,
             ItemRepository,
             IvkikItem};
use uuid::Uuid;

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

#[async_trait]
impl ItemRepository for SqliteIvkikRepository {
    async fn create_item(&self, item: IvkikItem) -> Result<IvkikItem, DomainError> {
        sqlx::query(
            r#"
            INSERT INTO ivkik_items (
                id, kind, parent_id, title, description, target_value, current_value,
                unit, position, status, aggregation, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(item.id.to_string())
        .bind(item.kind.as_str())
        .bind(item.parent_id.map(|id| id.to_string()))
        .bind(&item.title)
        .bind(&item.description)
        .bind(item.target_value)
        .bind(item.current_value)
        .bind(&item.unit)
        .bind(item.position)
        .bind(item.status.as_str())
        .bind(item.aggregation.as_str())
        .bind(item.created_at.to_rfc3339())
        .bind(item.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(item)
    }

    async fn get_item_by_id(&self, id: Uuid) -> Result<Option<IvkikItem>, DomainError> {
        let row = sqlx::query(
            r#"
            SELECT id, kind, parent_id, title, description, target_value, current_value,
                   unit, position, status, aggregation, created_at, updated_at
            FROM ivkik_items
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        match row {
            | Some(row) => Ok(Some(row_to_item(&row)?)),
            | None => Ok(None),
        }
    }

    async fn list_items(&self) -> Result<Vec<IvkikItem>, DomainError> {
        let rows = sqlx::query(
            r#"
            SELECT id, kind, parent_id, title, description, target_value, current_value,
                   unit, position, status, aggregation, created_at, updated_at
            FROM ivkik_items
            ORDER BY
              CASE kind
                WHEN 'identity' THEN 1
                WHEN 'vision' THEN 2
                WHEN 'kra' THEN 3
                WHEN 'igt' THEN 4
                WHEN 'kpi' THEN 5
                ELSE 99
              END,
              position,
              title
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        rows.iter().map(row_to_item).collect()
    }

    async fn update_item(&self, item: IvkikItem) -> Result<IvkikItem, DomainError> {
        sqlx::query(
            r#"
            UPDATE ivkik_items
            SET kind = ?, parent_id = ?, title = ?, description = ?, target_value = ?,
                current_value = ?, unit = ?, position = ?, status = ?, aggregation = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(item.kind.as_str())
        .bind(item.parent_id.map(|id| id.to_string()))
        .bind(&item.title)
        .bind(&item.description)
        .bind(item.target_value)
        .bind(item.current_value)
        .bind(&item.unit)
        .bind(item.position)
        .bind(item.status.as_str())
        .bind(item.aggregation.as_str())
        .bind(item.updated_at.to_rfc3339())
        .bind(item.id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(item)
    }

    async fn delete(&self, id: Uuid) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM ivkik_items WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn search_items(&self, query: &str) -> Result<Vec<IvkikItem>, DomainError> {
        let search_pattern = format!("%{}%", escape_like_pattern(query));
        let rows = sqlx::query(
            r#"
            SELECT id, kind, parent_id, title, description, target_value, current_value,
                   unit, position, status, aggregation, created_at, updated_at
            FROM ivkik_items
            WHERE title LIKE ?1 ESCAPE '\'
               OR description LIKE ?1 ESCAPE '\'
               OR unit LIKE ?1 ESCAPE '\'
               OR kind LIKE ?1 ESCAPE '\'
            ORDER BY position, title
            "#,
        )
        .bind(&search_pattern)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        rows.iter().map(row_to_item).collect()
    }
}
