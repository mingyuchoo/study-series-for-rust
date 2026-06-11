use async_trait::async_trait;
use chrono::{DateTime,
             Utc};
use domain::{DomainError,
             ItemKind,
             ItemRevision,
             ItemStatus,
             IvkikItem,
             IvkikRepository,
             KpiAggregation,
             KpiMeasurement};
use sqlx::{Row,
           SqlitePool,
           sqlite::SqliteRow};
use std::str::FromStr;
use uuid::Uuid;

pub struct SqliteIvkikRepository {
    pool: SqlitePool,
}

fn parse_datetime(value: &str) -> Result<DateTime<Utc>, DomainError> {
    DateTime::parse_from_rfc3339(value)
        .map_err(|e| DomainError::DatabaseError(e.to_string()))
        .map(|value| value.with_timezone(&Utc))
}

fn row_to_item(row: &SqliteRow) -> Result<IvkikItem, DomainError> {
    Ok(IvkikItem {
        id: Uuid::parse_str(row.get("id")).map_err(|e| DomainError::DatabaseError(e.to_string()))?,
        kind: ItemKind::from_str(row.get("kind")).map_err(DomainError::DatabaseError)?,
        parent_id: row
            .get::<Option<String>, _>("parent_id")
            .map(|id| Uuid::parse_str(&id).map_err(|e| DomainError::DatabaseError(e.to_string())))
            .transpose()?,
        title: row.get("title"),
        description: row.get("description"),
        target_value: row.get("target_value"),
        current_value: row.get("current_value"),
        unit: row.get("unit"),
        position: row.get("position"),
        status: ItemStatus::from_str(row.get("status")).map_err(DomainError::DatabaseError)?,
        aggregation: KpiAggregation::from_str(row.get("aggregation")).map_err(DomainError::DatabaseError)?,
        created_at: parse_datetime(row.get("created_at"))?,
        updated_at: parse_datetime(row.get("updated_at"))?,
    })
}

fn row_to_measurement(row: &SqliteRow) -> Result<KpiMeasurement, DomainError> {
    Ok(KpiMeasurement {
        id: Uuid::parse_str(row.get("id")).map_err(|e| DomainError::DatabaseError(e.to_string()))?,
        kpi_id: Uuid::parse_str(row.get("kpi_id")).map_err(|e| DomainError::DatabaseError(e.to_string()))?,
        value: row.get("value"),
        measured_at: parse_datetime(row.get("measured_at"))?,
        note: row.get("note"),
    })
}

fn row_to_revision(row: &SqliteRow) -> Result<ItemRevision, DomainError> {
    Ok(ItemRevision {
        id: Uuid::parse_str(row.get("id")).map_err(|e| DomainError::DatabaseError(e.to_string()))?,
        item_id: Uuid::parse_str(row.get("item_id")).map_err(|e| DomainError::DatabaseError(e.to_string()))?,
        field: row.get("field"),
        old_value: row.get("old_value"),
        new_value: row.get("new_value"),
        changed_at: parse_datetime(row.get("changed_at"))?,
    })
}

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

impl SqliteIvkikRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
        }
    }

    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query("PRAGMA foreign_keys = ON").execute(&self.pool).await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ivkik_items (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                parent_id TEXT REFERENCES ivkik_items(id) ON DELETE CASCADE,
                title TEXT NOT NULL,
                description TEXT,
                target_value REAL,
                current_value REAL,
                unit TEXT,
                position INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                aggregation TEXT NOT NULL DEFAULT 'latest',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // aggregation 컬럼이 없던 기존 데이터베이스를 위한 마이그레이션.
        let has_aggregation = sqlx::query("SELECT 1 FROM pragma_table_info('ivkik_items') WHERE name = 'aggregation'")
            .fetch_optional(&self.pool)
            .await?
            .is_some();
        if !has_aggregation {
            sqlx::query("ALTER TABLE ivkik_items ADD COLUMN aggregation TEXT NOT NULL DEFAULT 'latest'")
                .execute(&self.pool)
                .await?;
        }

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS kpi_measurements (
                id TEXT PRIMARY KEY,
                kpi_id TEXT NOT NULL REFERENCES ivkik_items(id) ON DELETE CASCADE,
                value REAL NOT NULL,
                measured_at TEXT NOT NULL,
                note TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS item_revisions (
                id TEXT PRIMARY KEY,
                item_id TEXT NOT NULL REFERENCES ivkik_items(id) ON DELETE CASCADE,
                field TEXT NOT NULL,
                old_value TEXT,
                new_value TEXT,
                changed_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_ivkik_items_parent ON ivkik_items(parent_id)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_ivkik_items_kind ON ivkik_items(kind)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_kpi_measurements_kpi ON kpi_measurements(kpi_id, measured_at)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_item_revisions_item ON item_revisions(item_id, changed_at)")
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

#[async_trait]
impl IvkikRepository for SqliteIvkikRepository {
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
                WHEN 'value' THEN 1
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

    async fn record_kpi_measurement(&self, measurement: KpiMeasurement) -> Result<KpiMeasurement, DomainError> {
        sqlx::query(
            r#"
            INSERT INTO kpi_measurements (id, kpi_id, value, measured_at, note)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(measurement.id.to_string())
        .bind(measurement.kpi_id.to_string())
        .bind(measurement.value)
        .bind(measurement.measured_at.to_rfc3339())
        .bind(&measurement.note)
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(measurement)
    }

    async fn list_kpi_measurements(&self, kpi_id: Uuid) -> Result<Vec<KpiMeasurement>, DomainError> {
        let rows = sqlx::query(
            r#"
            SELECT id, kpi_id, value, measured_at, note
            FROM kpi_measurements
            WHERE kpi_id = ?
            ORDER BY measured_at DESC
            "#,
        )
        .bind(kpi_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        rows.iter().map(row_to_measurement).collect()
    }

    async fn list_all_kpi_measurements(&self) -> Result<Vec<KpiMeasurement>, DomainError> {
        let rows = sqlx::query(
            r#"
            SELECT id, kpi_id, value, measured_at, note
            FROM kpi_measurements
            ORDER BY measured_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        rows.iter().map(row_to_measurement).collect()
    }

    async fn delete_kpi_measurement(&self, kpi_id: Uuid, measurement_id: Uuid) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM kpi_measurements WHERE id = ? AND kpi_id = ?")
            .bind(measurement_id.to_string())
            .bind(kpi_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use domain::IvkikRepository;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn repository() -> SqliteIvkikRepository {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool should be created");
        let repository = SqliteIvkikRepository::new(pool);
        repository.init().await.expect("ivkik tables should be created");
        repository
    }

    #[tokio::test]
    async fn creates_lists_searches_updates_and_deletes_items() {
        let repository = repository().await;
        let mut item = IvkikItem::new(domain::NewIvkikItem {
            kind: ItemKind::Identity,
            parent_id: None,
            title: "  Freedom  ".to_string(),
            description: Some("Primary filter".to_string()),
            target_value: None,
            current_value: None,
            unit: None,
            position: 0,
            aggregation: KpiAggregation::default(),
        });
        let id = item.id;

        repository.create_item(item.clone()).await.expect("item should be created");

        let items = repository.list_items().await.expect("items should be listed");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Freedom");

        let matches = repository.search_items("filter").await.expect("item should be searchable");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, id);

        item.update(domain::ItemPatch {
            title: Some("Financial freedom".to_string()),
            description: Some("  ".to_string()),
            ..domain::ItemPatch::default()
        });
        repository.update_item(item).await.expect("item should be updated");

        let updated = repository
            .get_item_by_id(id)
            .await
            .expect("item lookup should succeed")
            .expect("item should exist");
        assert_eq!(updated.title, "Financial freedom");
        assert_eq!(updated.description, None);

        repository.delete(id).await.expect("item should be deleted");
        let deleted = repository.get_item_by_id(id).await.expect("item lookup should succeed");
        assert_eq!(deleted, None);
    }

    #[tokio::test]
    async fn search_treats_like_wildcards_literally() {
        let repository = repository().await;
        repository
            .create_item(IvkikItem::new(domain::NewIvkikItem {
                kind: ItemKind::Identity,
                parent_id: None,
                title: "100% Ownership".to_string(),
                description: None,
                target_value: None,
                current_value: None,
                unit: None,
                position: 0,
                aggregation: KpiAggregation::default(),
            }))
            .await
            .expect("item should be created");
        repository
            .create_item(IvkikItem::new(domain::NewIvkikItem {
                kind: ItemKind::Identity,
                parent_id: None,
                title: "Identity Stream".to_string(),
                description: Some("Plain text".to_string()),
                target_value: None,
                current_value: None,
                unit: None,
                position: 1,
                aggregation: KpiAggregation::default(),
            }))
            .await
            .expect("item should be created");

        let matches = repository.search_items("%").await.expect("search should succeed");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].title, "100% Ownership");
    }

    #[tokio::test]
    async fn records_kpi_measurements() {
        let repository = repository().await;
        let kpi = repository
            .create_item(IvkikItem::new(domain::NewIvkikItem {
                kind: ItemKind::Kpi,
                parent_id: None,
                title: "Monthly recurring revenue".to_string(),
                description: None,
                target_value: Some(10_000.0),
                current_value: Some(0.0),
                unit: Some("USD".to_string()),
                position: 0,
                aggregation: KpiAggregation::Sum,
            }))
            .await
            .expect("kpi should be created");

        // aggregation 컬럼도 저장·복원되는지 함께 확인한다.
        let stored = repository
            .get_item_by_id(kpi.id)
            .await
            .expect("kpi lookup should succeed")
            .expect("kpi should exist");
        assert_eq!(stored.aggregation, KpiAggregation::Sum);

        let measurement = repository
            .record_kpi_measurement(KpiMeasurement::new(kpi.id, 1200.0, Some("First month".to_string())))
            .await
            .expect("measurement should be recorded");

        let measurements = repository.list_kpi_measurements(kpi.id).await.expect("measurements should be listed");
        assert_eq!(measurements, vec![measurement.clone()]);

        // 전체 조회는 KPI 구분 없이 모든 기록을 돌려준다.
        let all = repository.list_all_kpi_measurements().await.expect("all measurements should be listed");
        assert_eq!(all, vec![measurement.clone()]);

        // 다른 KPI의 id로는 지워지지 않아야 한다.
        repository
            .delete_kpi_measurement(Uuid::new_v4(), measurement.id)
            .await
            .expect("delete with wrong kpi id should succeed silently");
        assert_eq!(repository.list_kpi_measurements(kpi.id).await.unwrap().len(), 1);

        repository
            .delete_kpi_measurement(kpi.id, measurement.id)
            .await
            .expect("measurement should be deleted");
        assert!(repository.list_kpi_measurements(kpi.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn records_and_lists_item_revisions_newest_first() {
        let repository = repository().await;
        let item = repository
            .create_item(IvkikItem::new(domain::NewIvkikItem {
                kind: ItemKind::Kpi,
                parent_id: None,
                title: "Monthly commits".to_string(),
                description: None,
                target_value: Some(40.0),
                current_value: None,
                unit: Some("회".to_string()),
                position: 0,
                aggregation: KpiAggregation::Sum,
            }))
            .await
            .expect("kpi should be created");

        let first = ItemRevision::new(item.id, "target_value", Some("40".to_string()), Some("60".to_string()), Utc::now());
        let second = ItemRevision::new(
            item.id,
            "title",
            Some("Monthly commits".to_string()),
            Some("Weekly commits".to_string()),
            Utc::now() + chrono::Duration::seconds(1),
        );
        repository
            .record_item_revisions(vec![first.clone(), second.clone()])
            .await
            .expect("revisions should be recorded");

        let revisions = repository.list_item_revisions(item.id).await.expect("revisions should be listed");
        assert_eq!(revisions, vec![second, first]);

        // 항목을 지우면 이력도 함께 사라진다(ON DELETE CASCADE).
        repository.delete(item.id).await.expect("item should be deleted");
        assert!(repository.list_item_revisions(item.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn init_is_idempotent_and_migrates_missing_aggregation_column() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool should be created");

        // aggregation 컬럼이 없던 구버전 스키마를 흉내 낸다.
        sqlx::query(
            r#"
            CREATE TABLE ivkik_items (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                parent_id TEXT REFERENCES ivkik_items(id) ON DELETE CASCADE,
                title TEXT NOT NULL,
                description TEXT,
                target_value REAL,
                current_value REAL,
                unit TEXT,
                position INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("legacy table should be created");

        let repository = SqliteIvkikRepository::new(pool);
        repository.init().await.expect("init should migrate the legacy schema");
        repository.init().await.expect("init should stay idempotent");

        let items = repository.list_items().await.expect("legacy rows should be readable");
        assert!(items.is_empty());
    }
}
