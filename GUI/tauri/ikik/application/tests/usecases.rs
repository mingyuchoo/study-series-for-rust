//! Use case unit tests driven by in-memory mock repository ports.

use application::{CreateItemUseCase,
                  DeleteItemUseCase,
                  DeleteKpiMeasurementUseCase,
                  ListItemRevisionsUseCase,
                  ListItemsUseCase,
                  ListKpiMeasurementsUseCase,
                  RecordKpiMeasurementUseCase,
                  SearchItemsUseCase,
                  UpdateItemUseCase};
use async_trait::async_trait;
use domain::{DomainError,
             IkikItem,
             ItemKind,
             ItemPatch,
             ItemRepository,
             ItemRevision,
             ItemRevisionRepository,
             KpiAggregation,
             KpiMeasurement,
             KpiMeasurementRepository,
             NewIkikItem};
use std::{collections::HashMap,
          sync::{Arc,
                 Mutex}};
use uuid::Uuid;

#[derive(Default)]
struct MockIkikRepository {
    items: Mutex<HashMap<Uuid, IkikItem>>,
    measurements: Mutex<HashMap<Uuid, Vec<KpiMeasurement>>>,
    revisions: Mutex<Vec<ItemRevision>>,
}

impl MockIkikRepository {
    fn arc() -> Arc<Self> { Arc::new(Self::default()) }

    fn count(&self) -> usize { self.items.lock().unwrap().len() }
}

#[async_trait]
impl ItemRepository for MockIkikRepository {
    async fn create_item(&self, item: IkikItem) -> Result<IkikItem, DomainError> {
        self.items.lock().unwrap().insert(item.id, item.clone());
        Ok(item)
    }

    async fn get_item_by_id(&self, id: Uuid) -> Result<Option<IkikItem>, DomainError> { Ok(self.items.lock().unwrap().get(&id).cloned()) }

    async fn list_items(&self) -> Result<Vec<IkikItem>, DomainError> {
        let mut items: Vec<IkikItem> = self.items.lock().unwrap().values().cloned().collect();
        items.sort_by(|a, b| a.position.cmp(&b.position).then(a.title.cmp(&b.title)));
        Ok(items)
    }

    async fn update_item(&self, item: IkikItem) -> Result<IkikItem, DomainError> {
        self.items.lock().unwrap().insert(item.id, item.clone());
        Ok(item)
    }

    async fn delete(&self, id: Uuid) -> Result<(), DomainError> {
        self.items.lock().unwrap().remove(&id);
        Ok(())
    }

    async fn search_items(&self, query: &str) -> Result<Vec<IkikItem>, DomainError> {
        let query = query.to_lowercase();
        let matches = |value: &Option<String>| value.as_ref().is_some_and(|v| v.to_lowercase().contains(&query));
        let items = self
            .items
            .lock()
            .unwrap()
            .values()
            .filter(|item| item.title.to_lowercase().contains(&query) || matches(&item.description) || matches(&item.unit))
            .cloned()
            .collect();
        Ok(items)
    }
}

#[async_trait]
impl KpiMeasurementRepository for MockIkikRepository {
    async fn record_kpi_measurement(&self, measurement: KpiMeasurement) -> Result<KpiMeasurement, DomainError> {
        self.measurements
            .lock()
            .unwrap()
            .entry(measurement.kpi_id)
            .or_default()
            .push(measurement.clone());
        Ok(measurement)
    }

    async fn list_kpi_measurements(&self, kpi_id: Uuid) -> Result<Vec<KpiMeasurement>, DomainError> {
        // 실제 리포지토리처럼 최신 기록이 앞에 오도록 돌려준다.
        let mut measurements = self.measurements.lock().unwrap().get(&kpi_id).cloned().unwrap_or_default();
        measurements.reverse();
        Ok(measurements)
    }

    async fn list_all_kpi_measurements(&self) -> Result<Vec<KpiMeasurement>, DomainError> {
        let mut all: Vec<KpiMeasurement> = self.measurements.lock().unwrap().values().flatten().cloned().collect();
        all.sort_by_key(|measurement| std::cmp::Reverse(measurement.measured_at));
        Ok(all)
    }

    async fn delete_kpi_measurement(&self, kpi_id: Uuid, measurement_id: Uuid) -> Result<(), DomainError> {
        if let Some(measurements) = self.measurements.lock().unwrap().get_mut(&kpi_id) {
            measurements.retain(|measurement| measurement.id != measurement_id);
        }
        Ok(())
    }
}

#[async_trait]
impl ItemRevisionRepository for MockIkikRepository {
    async fn record_item_revisions(&self, revisions: Vec<ItemRevision>) -> Result<(), DomainError> {
        self.revisions.lock().unwrap().extend(revisions);
        Ok(())
    }

    async fn list_item_revisions(&self, item_id: Uuid) -> Result<Vec<ItemRevision>, DomainError> {
        let mut revisions: Vec<ItemRevision> = self
            .revisions
            .lock()
            .unwrap()
            .iter()
            .filter(|revision| revision.item_id == item_id)
            .cloned()
            .collect();
        revisions.sort_by_key(|revision| std::cmp::Reverse(revision.changed_at));
        Ok(revisions)
    }
}

fn draft(kind: ItemKind, parent_id: Option<Uuid>, title: &str) -> NewIkikItem {
    NewIkikItem {
        kind,
        parent_id,
        title: title.to_string(),
        description: None,
        target_value: None,
        current_value: None,
        unit: None,
        position: 0,
        aggregation: KpiAggregation::default(),
        due_date: None,
    }
}

fn kpi_draft(parent_id: Option<Uuid>, title: &str) -> NewIkikItem {
    NewIkikItem {
        target_value: Some(10_000.0),
        current_value: Some(0.0),
        unit: Some("USD".to_string()),
        ..draft(ItemKind::Kpi, parent_id, title)
    }
}

#[tokio::test]
async fn create_rejects_blank_title() {
    let repository = MockIkikRepository::arc();
    let use_case = CreateItemUseCase::new(repository.clone());

    let result = use_case.execute(draft(ItemKind::Identity, None, "   ")).await;

    assert!(matches!(result, Err(DomainError::InvalidIkikData(_))));
    assert_eq!(repository.count(), 0);
}

#[tokio::test]
async fn create_rejects_invalid_parent_hierarchy() {
    let repository = MockIkikRepository::arc();
    let create = CreateItemUseCase::new(repository.clone());
    let identity = create
        .execute(draft(ItemKind::Identity, None, "Freedom"))
        .await
        .expect("identity should be created");

    let result = create.execute(draft(ItemKind::Igt, Some(identity.id), "Sales engine")).await;

    assert!(matches!(result, Err(DomainError::InvalidIkikData(_))));
    assert_eq!(repository.count(), 1);
}

#[tokio::test]
async fn create_rejects_due_date_on_identity() {
    let repository = MockIkikRepository::arc();
    let create = CreateItemUseCase::new(repository.clone());

    let result = create
        .execute(NewIkikItem {
            due_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 30),
            ..draft(ItemKind::Identity, None, "Freedom")
        })
        .await;

    assert!(matches!(result, Err(DomainError::InvalidIkikData(_))));
    assert_eq!(repository.count(), 0);
}

#[tokio::test]
async fn update_records_due_date_revision() {
    let repository = MockIkikRepository::arc();
    let create = CreateItemUseCase::new(repository.clone());
    let identity = create.execute(draft(ItemKind::Identity, None, "Freedom")).await.unwrap();
    let kra = create.execute(draft(ItemKind::Kra, Some(identity.id), "Revenue")).await.unwrap();

    let due = chrono::NaiveDate::from_ymd_opt(2026, 6, 30);
    let updated = UpdateItemUseCase::new(repository.clone())
        .execute(
            kra.id,
            ItemPatch {
                due_date: Some(due),
                ..ItemPatch::default()
            },
        )
        .await
        .expect("due date update should succeed");
    assert_eq!(updated.due_date, due);

    let revisions = ListItemRevisionsUseCase::new(repository)
        .execute(kra.id)
        .await
        .expect("revisions should be listed");
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].field, "due_date");
    assert_eq!(revisions[0].old_value, None);
    assert_eq!(revisions[0].new_value.as_deref(), Some("2026-06-30"));
}

#[tokio::test]
async fn create_persists_valid_hierarchy() {
    let repository = MockIkikRepository::arc();
    let create = CreateItemUseCase::new(repository.clone());

    let identity = create
        .execute(draft(ItemKind::Identity, None, "Freedom"))
        .await
        .expect("identity should be created");
    let kra = create
        .execute(draft(ItemKind::Kra, Some(identity.id), "Audience growth"))
        .await
        .expect("kra should be created");
    let igt = create
        .execute(draft(ItemKind::Igt, Some(kra.id), "Publish offer"))
        .await
        .expect("igt should be created");
    let kpi = create.execute(kpi_draft(Some(igt.id), "Monthly revenue")).await.expect("kpi should be created");

    assert_eq!(kpi.parent_id, Some(igt.id));
    assert_eq!(repository.count(), 4);
}

#[tokio::test]
async fn update_returns_not_found_for_missing_item() {
    let repository = MockIkikRepository::arc();

    let update_result = UpdateItemUseCase::new(repository)
        .execute(
            Uuid::new_v4(),
            ItemPatch {
                title: Some("New title".to_string()),
                ..ItemPatch::default()
            },
        )
        .await;
    assert!(matches!(update_result, Err(DomainError::ItemNotFound)));
}

#[tokio::test]
async fn update_mutates_existing_item() {
    let repository = MockIkikRepository::arc();
    let created = CreateItemUseCase::new(repository.clone())
        .execute(draft(ItemKind::Identity, None, "Freedom"))
        .await
        .expect("item should be created");

    let updated = UpdateItemUseCase::new(repository.clone())
        .execute(
            created.id,
            ItemPatch {
                title: Some("Creative freedom".to_string()),
                description: Some("Build a calm operating system".to_string()),
                position: Some(3),
                ..ItemPatch::default()
            },
        )
        .await
        .expect("item should be updated");

    assert_eq!(updated.title, "Creative freedom");
    assert_eq!(updated.description, Some("Build a calm operating system".to_string()));
    assert_eq!(updated.position, 3);
    assert!(updated.updated_at >= created.updated_at);
}

#[tokio::test]
async fn update_records_revisions_for_changed_fields_only() {
    let repository = MockIkikRepository::arc();
    let created = CreateItemUseCase::new(repository.clone())
        .execute(draft(ItemKind::Identity, None, "Freedom"))
        .await
        .expect("item should be created");

    let update = UpdateItemUseCase::new(repository.clone());
    update
        .execute(
            created.id,
            ItemPatch {
                title: Some("Creative freedom".to_string()),
                description: Some("Calm operating system".to_string()),
                ..ItemPatch::default()
            },
        )
        .await
        .expect("item should be updated");

    let revisions = ListItemRevisionsUseCase::new(repository.clone())
        .execute(created.id)
        .await
        .expect("revisions should be listed");
    let mut fields: Vec<&str> = revisions.iter().map(|revision| revision.field.as_str()).collect();
    fields.sort_unstable();
    assert_eq!(fields, vec!["description", "title"]);

    // 아무것도 안 바뀐 수정은 이력을 남기지 않는다.
    update
        .execute(
            created.id,
            ItemPatch {
                title: Some("Creative freedom".to_string()),
                ..ItemPatch::default()
            },
        )
        .await
        .expect("no-op update should succeed");
    assert_eq!(ListItemRevisionsUseCase::new(repository).execute(created.id).await.unwrap().len(), 2);
}

#[tokio::test]
async fn delete_removes_item() {
    let repository = MockIkikRepository::arc();
    let created = CreateItemUseCase::new(repository.clone())
        .execute(draft(ItemKind::Identity, None, "Freedom"))
        .await
        .expect("item should be created");

    DeleteItemUseCase::new(repository.clone())
        .execute(created.id)
        .await
        .expect("item should be deleted");

    assert_eq!(repository.count(), 0);
}

#[tokio::test]
async fn list_and_search_return_expected_items() {
    let repository = MockIkikRepository::arc();
    let create = CreateItemUseCase::new(repository.clone());
    create
        .execute(NewIkikItem {
            description: Some("Primary filter".to_string()),
            ..draft(ItemKind::Identity, None, "Freedom")
        })
        .await
        .unwrap();
    create
        .execute(NewIkikItem {
            description: Some("Skill growth".to_string()),
            position: 1,
            ..draft(ItemKind::Identity, None, "Mastery")
        })
        .await
        .unwrap();

    let all = ListItemsUseCase::new(repository.clone()).execute().await.expect("list should succeed");
    assert_eq!(all.len(), 2);

    let matches = SearchItemsUseCase::new(repository).execute("skill").await.expect("search should succeed");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].title, "Mastery");
}

#[tokio::test]
async fn record_kpi_measurement_updates_current_value() {
    let repository = MockIkikRepository::arc();
    let kpi = CreateItemUseCase::new(repository.clone())
        .execute(kpi_draft(Some(Uuid::new_v4()), "Monthly revenue"))
        .await;
    assert!(matches!(kpi, Err(DomainError::ItemNotFound)));

    let identity = CreateItemUseCase::new(repository.clone())
        .execute(draft(ItemKind::Identity, None, "Freedom"))
        .await
        .unwrap();
    let kra = CreateItemUseCase::new(repository.clone())
        .execute(draft(ItemKind::Kra, Some(identity.id), "Revenue"))
        .await
        .unwrap();
    let igt = CreateItemUseCase::new(repository.clone())
        .execute(draft(ItemKind::Igt, Some(kra.id), "Publish offer"))
        .await
        .unwrap();
    let kpi = CreateItemUseCase::new(repository.clone())
        .execute(kpi_draft(Some(igt.id), "Monthly revenue"))
        .await
        .unwrap();

    let measurement = RecordKpiMeasurementUseCase::new(repository.clone())
        .execute(kpi.id, 1200.0, Some("First month".to_string()))
        .await
        .expect("measurement should be recorded");

    let updated = repository.get_item_by_id(kpi.id).await.unwrap().unwrap();
    let measurements = repository.list_kpi_measurements(kpi.id).await.unwrap();
    assert_eq!(updated.current_value, Some(1200.0));
    assert_eq!(measurements, vec![measurement]);
}

/// Identity → … → Key Performance Indicator 계층을 만들어 측정 기록 테스트가 쓸
/// Key Performance Indicator를 돌려준다.
async fn seeded_kpi(repository: &Arc<MockIkikRepository>, aggregation: KpiAggregation) -> IkikItem {
    let create = CreateItemUseCase::new(repository.clone());
    let identity = create.execute(draft(ItemKind::Identity, None, "Freedom")).await.unwrap();
    let kra = create.execute(draft(ItemKind::Kra, Some(identity.id), "Revenue")).await.unwrap();
    let igt = create.execute(draft(ItemKind::Igt, Some(kra.id), "Publish offer")).await.unwrap();
    create
        .execute(NewIkikItem {
            aggregation,
            ..kpi_draft(Some(igt.id), "Monthly revenue")
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn sum_aggregation_accumulates_measurements_into_current_value() {
    let repository = MockIkikRepository::arc();
    let kpi = seeded_kpi(&repository, KpiAggregation::Sum).await;
    let record = RecordKpiMeasurementUseCase::new(repository.clone());

    record.execute(kpi.id, 3.0, Some("Week 1".to_string())).await.unwrap();
    record.execute(kpi.id, 4.0, None).await.unwrap();

    let updated = repository.get_item_by_id(kpi.id).await.unwrap().unwrap();
    assert_eq!(updated.current_value, Some(7.0));
}

#[tokio::test]
async fn average_aggregation_averages_measurements() {
    let repository = MockIkikRepository::arc();
    let kpi = seeded_kpi(&repository, KpiAggregation::Average).await;
    let record = RecordKpiMeasurementUseCase::new(repository.clone());

    record.execute(kpi.id, 6.0, None).await.unwrap();
    record.execute(kpi.id, 8.0, None).await.unwrap();

    let updated = repository.get_item_by_id(kpi.id).await.unwrap().unwrap();
    assert_eq!(updated.current_value, Some(7.0));
}

#[tokio::test]
async fn deleting_a_measurement_recomputes_and_clears_when_empty() {
    let repository = MockIkikRepository::arc();
    let kpi = seeded_kpi(&repository, KpiAggregation::Sum).await;
    let record = RecordKpiMeasurementUseCase::new(repository.clone());

    let first = record.execute(kpi.id, 3.0, None).await.unwrap();
    let second = record.execute(kpi.id, 4.0, None).await.unwrap();

    let delete = DeleteKpiMeasurementUseCase::new(repository.clone());
    delete.execute(kpi.id, first.id).await.expect("measurement should be deleted");
    assert_eq!(repository.get_item_by_id(kpi.id).await.unwrap().unwrap().current_value, Some(4.0));

    // 마지막 기록을 지우면 현재값도 비워진다.
    delete.execute(kpi.id, second.id).await.expect("measurement should be deleted");
    assert_eq!(repository.get_item_by_id(kpi.id).await.unwrap().unwrap().current_value, None);
}

#[tokio::test]
async fn switching_aggregation_recomputes_current_value() {
    let repository = MockIkikRepository::arc();
    let kpi = seeded_kpi(&repository, KpiAggregation::Sum).await;
    let record = RecordKpiMeasurementUseCase::new(repository.clone());

    record.execute(kpi.id, 3.0, None).await.unwrap();
    record.execute(kpi.id, 4.0, None).await.unwrap();

    let updated = UpdateItemUseCase::new(repository.clone())
        .execute(
            kpi.id,
            ItemPatch {
                aggregation: Some(KpiAggregation::Latest),
                ..ItemPatch::default()
            },
        )
        .await
        .expect("aggregation switch should succeed");

    // 합계(7.0)가 아니라 마지막 기록(4.0)으로 다시 집계된다.
    assert_eq!(updated.aggregation, KpiAggregation::Latest);
    assert_eq!(updated.current_value, Some(4.0));
}

#[tokio::test]
async fn manual_current_value_survives_update_when_no_measurements_exist() {
    let repository = MockIkikRepository::arc();
    let kpi = seeded_kpi(&repository, KpiAggregation::Sum).await;

    let updated = UpdateItemUseCase::new(repository.clone())
        .execute(
            kpi.id,
            ItemPatch {
                current_value: Some(Some(42.0)),
                ..ItemPatch::default()
            },
        )
        .await
        .expect("manual update should succeed");

    assert_eq!(updated.current_value, Some(42.0));
}

#[tokio::test]
async fn list_kpi_measurements_rejects_non_kpi_items() {
    let repository = MockIkikRepository::arc();
    let identity = CreateItemUseCase::new(repository.clone())
        .execute(draft(ItemKind::Identity, None, "Freedom"))
        .await
        .unwrap();

    let result = ListKpiMeasurementsUseCase::new(repository.clone()).execute(identity.id).await;
    assert!(matches!(result, Err(DomainError::InvalidIkikData(_))));

    let missing = ListKpiMeasurementsUseCase::new(repository).execute(Uuid::new_v4()).await;
    assert!(matches!(missing, Err(DomainError::ItemNotFound)));
}
