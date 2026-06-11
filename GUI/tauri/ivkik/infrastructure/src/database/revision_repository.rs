use super::{SqliteIvkikRepository,
            mapper::row_to_revision};
use async_trait::async_trait;
use domain::{DomainError,
             ItemRevision,
             ItemRevisionRepository};
use uuid::Uuid;

#[async_trait]
impl ItemRevisionRepository for SqliteIvkikRepository {
    async fn record_item_revisions(&self, revisions: Vec<ItemRevision>) -> Result<(), DomainError> {
        for revision in revisions {
            sqlx::query(
                r#"
                INSERT INTO item_revisions (id, item_id, field, old_value, new_value, changed_at)
                VALUES (?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(revision.id.to_string())
            .bind(revision.item_id.to_string())
            .bind(&revision.field)
            .bind(&revision.old_value)
            .bind(&revision.new_value)
            .bind(revision.changed_at.to_rfc3339())
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        }

        Ok(())
    }

    async fn list_item_revisions(&self, item_id: Uuid) -> Result<Vec<ItemRevision>, DomainError> {
        let rows = sqlx::query(
            r#"
            SELECT id, item_id, field, old_value, new_value, changed_at
            FROM item_revisions
            WHERE item_id = ?
            ORDER BY changed_at DESC, field
            "#,
        )
        .bind(item_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        rows.iter().map(row_to_revision).collect()
    }
}
