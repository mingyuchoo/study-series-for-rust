#[path = "item_repository.rs"]
mod item_repository;
#[path = "mapper.rs"]
mod mapper;
#[path = "measurement_repository.rs"]
mod measurement_repository;
#[path = "revision_repository.rs"]
mod revision_repository;
#[path = "schema.rs"]
mod schema;

use sqlx::SqlitePool;

pub struct SqliteIkikRepository {
    pub(crate) pool: SqlitePool,
}

impl SqliteIkikRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
        }
    }

    pub async fn init(&self) -> Result<(), sqlx::Error> { schema::init(&self.pool).await }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain::{IkikItem,
                 ItemKind,
                 ItemRepository,
                 ItemRevision,
                 ItemRevisionRepository,
                 KpiAggregation,
                 KpiMeasurement,
                 KpiMeasurementRepository};
    use sqlx::sqlite::SqlitePoolOptions;
    use uuid::Uuid;

    async fn repository() -> SqliteIkikRepository {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool should be created");
        let repository = SqliteIkikRepository::new(pool);
        repository.init().await.expect("ikik tables should be created");
        repository
    }

    #[tokio::test]
    async fn creates_lists_searches_updates_and_deletes_items() {
        let repository = repository().await;
        let mut item = IkikItem::new(domain::NewIkikItem {
            kind: ItemKind::Identity,
            parent_id: None,
            title: "  Freedom  ".to_string(),
            description: Some("Primary filter".to_string()),
            target_value: None,
            current_value: None,
            unit: None,
            position: 0,
            aggregation: KpiAggregation::default(),
            due_date: None,
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
    async fn lists_items_in_ikik_kind_order() {
        let repository = repository().await;
        for (kind, title) in [
            (ItemKind::Kpi, "Key Performance Indicator"),
            (ItemKind::Igt, "Income Generating Task"),
            (ItemKind::Kra, "Key Result Area"),
            (ItemKind::Identity, "Identity"),
        ] {
            repository
                .create_item(IkikItem::new(domain::NewIkikItem {
                    kind,
                    parent_id: None,
                    title: title.to_string(),
                    description: None,
                    target_value: None,
                    current_value: None,
                    unit: None,
                    position: 0,
                    aggregation: KpiAggregation::default(),
                    due_date: None,
                }))
                .await
                .expect("item should be created");
        }

        let kinds: Vec<ItemKind> = repository.list_items().await.unwrap().into_iter().map(|item| item.kind).collect();

        assert_eq!(kinds, vec![ItemKind::Identity, ItemKind::Kra, ItemKind::Igt, ItemKind::Kpi]);
    }

    #[tokio::test]
    async fn search_treats_like_wildcards_literally() {
        let repository = repository().await;
        repository
            .create_item(IkikItem::new(domain::NewIkikItem {
                kind: ItemKind::Identity,
                parent_id: None,
                title: "100% Ownership".to_string(),
                description: None,
                target_value: None,
                current_value: None,
                unit: None,
                position: 0,
                aggregation: KpiAggregation::default(),
                due_date: None,
            }))
            .await
            .expect("item should be created");
        repository
            .create_item(IkikItem::new(domain::NewIkikItem {
                kind: ItemKind::Identity,
                parent_id: None,
                title: "Identity Stream".to_string(),
                description: Some("Plain text".to_string()),
                target_value: None,
                current_value: None,
                unit: None,
                position: 1,
                aggregation: KpiAggregation::default(),
                due_date: None,
            }))
            .await
            .expect("item should be created");

        let matches = repository.search_items("%").await.expect("search should succeed");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].title, "100% Ownership");
    }

    #[tokio::test]
    async fn stores_and_round_trips_due_date() {
        let repository = repository().await;
        let due = chrono::NaiveDate::from_ymd_opt(2026, 7, 31);
        let item = repository
            .create_item(IkikItem::new(domain::NewIkikItem {
                kind: ItemKind::Igt,
                parent_id: None,
                title: "Launch funnel".to_string(),
                description: None,
                target_value: None,
                current_value: None,
                unit: None,
                position: 0,
                aggregation: KpiAggregation::default(),
                due_date: due,
            }))
            .await
            .expect("item should be created");

        let stored = repository
            .get_item_by_id(item.id)
            .await
            .expect("item lookup should succeed")
            .expect("item should exist");
        assert_eq!(stored.due_date, due);

        // 마감 비우기(Some(None) 패치)도 저장돼야 한다.
        let mut cleared = stored.clone();
        cleared.update(domain::ItemPatch {
            due_date: Some(None),
            ..domain::ItemPatch::default()
        });
        repository.update_item(cleared).await.expect("item should be updated");
        let reloaded = repository
            .get_item_by_id(item.id)
            .await
            .expect("item lookup should succeed")
            .expect("item should exist");
        assert_eq!(reloaded.due_date, None);
    }

    #[tokio::test]
    async fn records_kpi_measurements() {
        let repository = repository().await;
        let kpi = repository
            .create_item(IkikItem::new(domain::NewIkikItem {
                kind: ItemKind::Kpi,
                parent_id: None,
                title: "Monthly recurring revenue".to_string(),
                description: None,
                target_value: Some(10_000.0),
                current_value: Some(0.0),
                unit: Some("USD".to_string()),
                position: 0,
                aggregation: KpiAggregation::Sum,
                due_date: None,
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

        // 전체 조회는 Key Performance Indicator 구분 없이 모든 기록을 돌려준다.
        let all = repository.list_all_kpi_measurements().await.expect("all measurements should be listed");
        assert_eq!(all, vec![measurement.clone()]);

        // 다른 Key Performance Indicator의 id로는 지워지지 않아야 한다.
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
            .create_item(IkikItem::new(domain::NewIkikItem {
                kind: ItemKind::Kpi,
                parent_id: None,
                title: "Monthly commits".to_string(),
                description: None,
                target_value: Some(40.0),
                current_value: None,
                unit: Some("회".to_string()),
                position: 0,
                aggregation: KpiAggregation::Sum,
                due_date: None,
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
    async fn init_is_idempotent_and_adds_missing_due_date_column() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool should be created");

        // due_date 컬럼이 없던 구버전 스키마를 흉내 낸다.
        sqlx::query(
            r#"
            CREATE TABLE ikik_items (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                parent_id TEXT REFERENCES ikik_items(id) ON DELETE CASCADE,
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
        .execute(&pool)
        .await
        .expect("legacy table should be created");

        let repository = SqliteIkikRepository::new(pool);
        repository.init().await.expect("init should migrate the legacy schema");
        repository.init().await.expect("init should stay idempotent");

        let items = repository.list_items().await.expect("legacy rows should be readable");
        assert!(items.is_empty());
    }
}
