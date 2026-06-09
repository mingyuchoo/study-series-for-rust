//! Use case unit tests driven by an in-memory mock `VvkikRepository`.

use application::{CreateItemUseCase,
                  DeleteItemUseCase,
                  GetItemUseCase,
                  ListItemsUseCase,
                  RecordKpiMeasurementUseCase,
                  SearchItemsUseCase,
                  UpdateItemUseCase};
use async_trait::async_trait;
use domain::{DomainError,
             ItemKind,
             KpiMeasurement,
             VvkikItem,
             VvkikRepository};
use std::{collections::HashMap,
          sync::{Arc,
                 Mutex}};
use uuid::Uuid;

#[derive(Default)]
struct MockVvkikRepository {
    items: Mutex<HashMap<Uuid, VvkikItem>>,
    measurements: Mutex<HashMap<Uuid, Vec<KpiMeasurement>>>,
}

impl MockVvkikRepository {
    fn arc() -> Arc<Self> { Arc::new(Self::default()) }

    fn count(&self) -> usize { self.items.lock().unwrap().len() }
}

#[async_trait]
impl VvkikRepository for MockVvkikRepository {
    async fn create_item(&self, item: VvkikItem) -> Result<VvkikItem, DomainError> {
        self.items.lock().unwrap().insert(item.id, item.clone());
        Ok(item)
    }

    async fn get_item_by_id(&self, id: Uuid) -> Result<Option<VvkikItem>, DomainError> { Ok(self.items.lock().unwrap().get(&id).cloned()) }

    async fn list_items(&self) -> Result<Vec<VvkikItem>, DomainError> {
        let mut items: Vec<VvkikItem> = self.items.lock().unwrap().values().cloned().collect();
        items.sort_by(|a, b| a.position.cmp(&b.position).then(a.title.cmp(&b.title)));
        Ok(items)
    }

    async fn update_item(&self, item: VvkikItem) -> Result<VvkikItem, DomainError> {
        self.items.lock().unwrap().insert(item.id, item.clone());
        Ok(item)
    }

    async fn delete(&self, id: Uuid) -> Result<(), DomainError> {
        self.items.lock().unwrap().remove(&id);
        Ok(())
    }

    async fn search_items(&self, query: &str) -> Result<Vec<VvkikItem>, DomainError> {
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
        Ok(self.measurements.lock().unwrap().get(&kpi_id).cloned().unwrap_or_default())
    }
}

#[tokio::test]
async fn create_rejects_blank_title() {
    let repository = MockVvkikRepository::arc();
    let use_case = CreateItemUseCase::new(repository.clone());

    let result = use_case.execute(ItemKind::Value, None, "   ".to_string(), None, None, None, None, 0).await;

    assert!(matches!(result, Err(DomainError::InvalidVvkikData(_))));
    assert_eq!(repository.count(), 0);
}

#[tokio::test]
async fn create_rejects_invalid_parent_hierarchy() {
    let repository = MockVvkikRepository::arc();
    let create = CreateItemUseCase::new(repository.clone());
    let value = create
        .execute(ItemKind::Value, None, "Freedom".to_string(), None, None, None, None, 0)
        .await
        .expect("value should be created");

    let result = create
        .execute(ItemKind::Kra, Some(value.id), "Sales engine".to_string(), None, None, None, None, 0)
        .await;

    assert!(matches!(result, Err(DomainError::InvalidVvkikData(_))));
    assert_eq!(repository.count(), 1);
}

#[tokio::test]
async fn create_persists_valid_hierarchy() {
    let repository = MockVvkikRepository::arc();
    let create = CreateItemUseCase::new(repository.clone());

    let value = create
        .execute(ItemKind::Value, None, "Freedom".to_string(), None, None, None, None, 0)
        .await
        .expect("value should be created");
    let vision = create
        .execute(ItemKind::Vision, Some(value.id), "Independent studio".to_string(), None, None, None, None, 0)
        .await
        .expect("vision should be created");
    let kra = create
        .execute(ItemKind::Kra, Some(vision.id), "Audience growth".to_string(), None, None, None, None, 0)
        .await
        .expect("kra should be created");
    let igt = create
        .execute(ItemKind::Igt, Some(kra.id), "Publish offer".to_string(), None, None, None, None, 0)
        .await
        .expect("igt should be created");
    let kpi = create
        .execute(
            ItemKind::Kpi,
            Some(igt.id),
            "Monthly revenue".to_string(),
            None,
            Some(10_000.0),
            Some(0.0),
            Some("USD".to_string()),
            0,
        )
        .await
        .expect("kpi should be created");

    assert_eq!(kpi.parent_id, Some(igt.id));
    assert_eq!(repository.count(), 5);
}

#[tokio::test]
async fn get_and_update_return_not_found_for_missing_item() {
    let repository = MockVvkikRepository::arc();

    let get_result = GetItemUseCase::new(repository.clone()).execute(Uuid::new_v4()).await;
    assert!(matches!(get_result, Err(DomainError::ItemNotFound)));

    let update_result = UpdateItemUseCase::new(repository)
        .execute(Uuid::new_v4(), None, None, Some("New title".to_string()), None, None, None, None, None, None)
        .await;
    assert!(matches!(update_result, Err(DomainError::ItemNotFound)));
}

#[tokio::test]
async fn update_mutates_existing_item() {
    let repository = MockVvkikRepository::arc();
    let created = CreateItemUseCase::new(repository.clone())
        .execute(ItemKind::Value, None, "Freedom".to_string(), None, None, None, None, 0)
        .await
        .expect("item should be created");

    let updated = UpdateItemUseCase::new(repository.clone())
        .execute(
            created.id,
            None,
            None,
            Some("Creative freedom".to_string()),
            Some("Build a calm operating system".to_string()),
            None,
            None,
            None,
            Some(3),
            None,
        )
        .await
        .expect("item should be updated");

    assert_eq!(updated.title, "Creative freedom");
    assert_eq!(updated.description, Some("Build a calm operating system".to_string()));
    assert_eq!(updated.position, 3);
    assert!(updated.updated_at >= created.updated_at);
}

#[tokio::test]
async fn delete_removes_item() {
    let repository = MockVvkikRepository::arc();
    let created = CreateItemUseCase::new(repository.clone())
        .execute(ItemKind::Value, None, "Freedom".to_string(), None, None, None, None, 0)
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
    let repository = MockVvkikRepository::arc();
    let create = CreateItemUseCase::new(repository.clone());
    create
        .execute(
            ItemKind::Value,
            None,
            "Freedom".to_string(),
            Some("Primary filter".to_string()),
            None,
            None,
            None,
            0,
        )
        .await
        .unwrap();
    create
        .execute(
            ItemKind::Value,
            None,
            "Mastery".to_string(),
            Some("Skill growth".to_string()),
            None,
            None,
            None,
            1,
        )
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
    let repository = MockVvkikRepository::arc();
    let kpi = CreateItemUseCase::new(repository.clone())
        .execute(
            ItemKind::Kpi,
            Some(Uuid::new_v4()),
            "Monthly revenue".to_string(),
            None,
            Some(10_000.0),
            Some(0.0),
            Some("USD".to_string()),
            0,
        )
        .await;
    assert!(matches!(kpi, Err(DomainError::ItemNotFound)));

    let value = CreateItemUseCase::new(repository.clone())
        .execute(ItemKind::Value, None, "Freedom".to_string(), None, None, None, None, 0)
        .await
        .unwrap();
    let vision = CreateItemUseCase::new(repository.clone())
        .execute(ItemKind::Vision, Some(value.id), "Independent studio".to_string(), None, None, None, None, 0)
        .await
        .unwrap();
    let kra = CreateItemUseCase::new(repository.clone())
        .execute(ItemKind::Kra, Some(vision.id), "Revenue".to_string(), None, None, None, None, 0)
        .await
        .unwrap();
    let kpi = CreateItemUseCase::new(repository.clone())
        .execute(
            ItemKind::Kpi,
            Some(kra.id),
            "Monthly revenue".to_string(),
            None,
            Some(10_000.0),
            Some(0.0),
            Some("USD".to_string()),
            0,
        )
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
