use crate::models::*;
use application::*;
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

pub struct AppState {
    pub create_item_use_case: Arc<CreateItemUseCase>,
    pub list_items_use_case: Arc<ListItemsUseCase>,
    pub update_item_use_case: Arc<UpdateItemUseCase>,
    pub delete_item_use_case: Arc<DeleteItemUseCase>,
    pub search_items_use_case: Arc<SearchItemsUseCase>,
    pub record_kpi_measurement_use_case: Arc<RecordKpiMeasurementUseCase>,
    pub list_kpi_measurements_use_case: Arc<ListKpiMeasurementsUseCase>,
    pub delete_kpi_measurement_use_case: Arc<DeleteKpiMeasurementUseCase>,
    pub list_all_kpi_measurements_use_case: Arc<ListAllKpiMeasurementsUseCase>,
    pub list_item_revisions_use_case: Arc<ListItemRevisionsUseCase>,
}

fn parse_id(id: &str) -> Result<Uuid, ApiError> { Uuid::parse_str(id).map_err(|e| ApiError::invalid_id(format!("Invalid IVKIK item id: {e}"))) }

fn parse_optional_id(id: Option<String>) -> Result<Option<Uuid>, ApiError> { id.as_deref().map(parse_id).transpose() }

#[tauri::command]
pub async fn create_item(state: State<'_, AppState>, request: CreateItemRequest) -> Result<IvkikItemDto, ApiError> {
    let parent_id = parse_optional_id(request.parent_id)?;

    state
        .create_item_use_case
        .execute(domain::NewIvkikItem {
            kind: request.kind,
            parent_id,
            title: request.title,
            description: request.description,
            target_value: request.target_value,
            current_value: request.current_value,
            unit: request.unit,
            position: request.position.unwrap_or_default(),
            aggregation: request.aggregation,
        })
        .await
        .map(item_to_dto)
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn list_items(state: State<'_, AppState>) -> Result<Vec<IvkikItemDto>, ApiError> {
    state
        .list_items_use_case
        .execute()
        .await
        .map(|items| items.into_iter().map(item_to_dto).collect())
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn update_item(state: State<'_, AppState>, request: UpdateItemRequest) -> Result<IvkikItemDto, ApiError> {
    let uuid = parse_id(&request.id)?;
    let parent_id = request.parent_id.map(parse_optional_id).transpose()?;

    state
        .update_item_use_case
        .execute(
            uuid,
            domain::ItemPatch {
                kind: request.kind,
                parent_id,
                title: request.title,
                description: request.description,
                target_value: request.target_value,
                current_value: request.current_value,
                unit: request.unit,
                position: request.position,
                status: request.status,
                aggregation: request.aggregation,
            },
        )
        .await
        .map(item_to_dto)
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn delete_item(state: State<'_, AppState>, id: String) -> Result<(), ApiError> {
    let uuid = parse_id(&id)?;

    state.delete_item_use_case.execute(uuid).await.map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn search_items(state: State<'_, AppState>, query: String) -> Result<Vec<IvkikItemDto>, ApiError> {
    state
        .search_items_use_case
        .execute(&query)
        .await
        .map(|items| items.into_iter().map(item_to_dto).collect())
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn record_kpi_measurement(state: State<'_, AppState>, request: RecordKpiMeasurementRequest) -> Result<KpiMeasurementDto, ApiError> {
    let kpi_id = parse_id(&request.kpi_id)?;

    state
        .record_kpi_measurement_use_case
        .execute(kpi_id, request.value, request.note)
        .await
        .map(measurement_to_dto)
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn list_kpi_measurements(state: State<'_, AppState>, kpi_id: String) -> Result<Vec<KpiMeasurementDto>, ApiError> {
    let kpi_id = parse_id(&kpi_id)?;

    state
        .list_kpi_measurements_use_case
        .execute(kpi_id)
        .await
        .map(|measurements| measurements.into_iter().map(measurement_to_dto).collect())
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn list_all_kpi_measurements(state: State<'_, AppState>) -> Result<Vec<KpiMeasurementDto>, ApiError> {
    state
        .list_all_kpi_measurements_use_case
        .execute()
        .await
        .map(|measurements| measurements.into_iter().map(measurement_to_dto).collect())
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn list_item_revisions(state: State<'_, AppState>, item_id: String) -> Result<Vec<ItemRevisionDto>, ApiError> {
    let item_id = parse_id(&item_id)?;

    state
        .list_item_revisions_use_case
        .execute(item_id)
        .await
        .map(|revisions| revisions.into_iter().map(revision_to_dto).collect())
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn delete_kpi_measurement(state: State<'_, AppState>, kpi_id: String, measurement_id: String) -> Result<(), ApiError> {
    let kpi_id = parse_id(&kpi_id)?;
    let measurement_id = parse_id(&measurement_id)?;

    state
        .delete_kpi_measurement_use_case
        .execute(kpi_id, measurement_id)
        .await
        .map_err(domain_error_to_api)
}
