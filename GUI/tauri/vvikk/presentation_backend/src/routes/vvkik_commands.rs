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
}

fn parse_id(id: &str) -> Result<Uuid, ApiError> { Uuid::parse_str(id).map_err(|e| ApiError::invalid_id(format!("Invalid VVKIK item id: {e}"))) }

fn parse_optional_id(id: Option<String>) -> Result<Option<Uuid>, ApiError> { id.as_deref().map(parse_id).transpose() }

#[tauri::command]
pub async fn create_item(state: State<'_, AppState>, request: CreateItemRequest) -> Result<VvkikItemDto, ApiError> {
    let kind = parse_kind(&request.kind).map_err(ApiError::invalid_id)?;
    let parent_id = parse_optional_id(request.parent_id)?;

    state
        .create_item_use_case
        .execute(
            kind,
            parent_id,
            request.title,
            request.description,
            request.target_value,
            request.current_value,
            request.unit,
            request.position.unwrap_or_default(),
        )
        .await
        .map(VvkikItemDto::from)
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn list_items(state: State<'_, AppState>) -> Result<Vec<VvkikItemDto>, ApiError> {
    state
        .list_items_use_case
        .execute()
        .await
        .map(|items| items.into_iter().map(VvkikItemDto::from).collect())
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn update_item(state: State<'_, AppState>, request: UpdateItemRequest) -> Result<VvkikItemDto, ApiError> {
    let uuid = parse_id(&request.id)?;
    let kind = request.kind.as_deref().map(parse_kind).transpose().map_err(ApiError::invalid_id)?;
    let parent_id = request.parent_id.map(parse_optional_id).transpose()?;
    let status = request.status.as_deref().map(parse_status).transpose().map_err(ApiError::invalid_id)?;

    state
        .update_item_use_case
        .execute(
            uuid,
            kind,
            parent_id,
            request.title,
            request.description,
            request.target_value,
            request.current_value,
            request.unit,
            request.position,
            status,
        )
        .await
        .map(VvkikItemDto::from)
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn delete_item(state: State<'_, AppState>, id: String) -> Result<(), ApiError> {
    let uuid = parse_id(&id)?;

    state.delete_item_use_case.execute(uuid).await.map_err(ApiError::from)
}

#[tauri::command]
pub async fn search_items(state: State<'_, AppState>, query: String) -> Result<Vec<VvkikItemDto>, ApiError> {
    state
        .search_items_use_case
        .execute(&query)
        .await
        .map(|items| items.into_iter().map(VvkikItemDto::from).collect())
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn record_kpi_measurement(state: State<'_, AppState>, request: RecordKpiMeasurementRequest) -> Result<KpiMeasurementDto, ApiError> {
    let kpi_id = parse_id(&request.kpi_id)?;

    state
        .record_kpi_measurement_use_case
        .execute(kpi_id, request.value, request.note)
        .await
        .map(KpiMeasurementDto::from)
        .map_err(ApiError::from)
}
