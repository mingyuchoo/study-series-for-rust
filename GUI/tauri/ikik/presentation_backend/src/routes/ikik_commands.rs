use crate::models::*;
use application::*;
use domain::IkikRepository;
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

/// 커맨드가 공유하는 유일한 상태. 유스케이스는 전부 무상태라 커맨드가
/// 그 자리에서 만들고, 리포지토리만 여기서 들고 다닌다 — 유스케이스를
/// 추가해도 이 구조체는 변하지 않는다.
pub struct AppState {
    pub repository: Arc<dyn IkikRepository>,
}

fn parse_id(id: &str) -> Result<Uuid, ApiError> { Uuid::parse_str(id).map_err(|e| ApiError::invalid_id(format!("Invalid IKIK item id: {e}"))) }

fn parse_optional_id(id: Option<String>) -> Result<Option<Uuid>, ApiError> { id.as_deref().map(parse_id).transpose() }

/// 와이어의 "YYYY-MM-DD" 마감 기한을 도메인 날짜로 바꾼다.
fn parse_optional_due_date(due_date: Option<String>) -> Result<Option<chrono::NaiveDate>, ApiError> {
    due_date
        .map(|date| {
            chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                .map_err(|e| ApiError::new(contracts::ApiErrorKind::Validation, format!("Invalid due date '{date}': {e}")))
        })
        .transpose()
}

#[tauri::command]
pub async fn create_item(state: State<'_, AppState>, request: CreateItemRequest) -> Result<IkikItemDto, ApiError> {
    let parent_id = parse_optional_id(request.parent_id)?;
    let due_date = parse_optional_due_date(request.due_date)?;

    CreateItemUseCase::new(state.repository.clone())
        .execute(domain::NewIkikItem {
            kind: request.kind,
            parent_id,
            title: request.title,
            description: request.description,
            target_value: request.target_value,
            current_value: request.current_value,
            unit: request.unit,
            position: request.position.unwrap_or_default(),
            aggregation: request.aggregation,
            due_date,
        })
        .await
        .map(item_to_dto)
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn list_items(state: State<'_, AppState>) -> Result<Vec<IkikItemDto>, ApiError> {
    ListItemsUseCase::new(state.repository.clone())
        .execute()
        .await
        .map(|items| items.into_iter().map(item_to_dto).collect())
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn update_item(state: State<'_, AppState>, request: UpdateItemRequest) -> Result<IkikItemDto, ApiError> {
    let uuid = parse_id(&request.id)?;
    let parent_id = request.parent_id.map(parse_optional_id).transpose()?;
    let due_date = request.due_date.map(parse_optional_due_date).transpose()?;

    UpdateItemUseCase::new(state.repository.clone())
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
                due_date,
            },
        )
        .await
        .map(item_to_dto)
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn delete_item(state: State<'_, AppState>, id: String) -> Result<(), ApiError> {
    let uuid = parse_id(&id)?;

    DeleteItemUseCase::new(state.repository.clone())
        .execute(uuid)
        .await
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn search_items(state: State<'_, AppState>, query: String) -> Result<Vec<IkikItemDto>, ApiError> {
    SearchItemsUseCase::new(state.repository.clone())
        .execute(&query)
        .await
        .map(|items| items.into_iter().map(item_to_dto).collect())
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn record_kpi_measurement(state: State<'_, AppState>, request: RecordKpiMeasurementRequest) -> Result<KpiMeasurementDto, ApiError> {
    let kpi_id = parse_id(&request.kpi_id)?;

    RecordKpiMeasurementUseCase::new(state.repository.clone())
        .execute(kpi_id, request.value, request.note)
        .await
        .map(measurement_to_dto)
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn list_kpi_measurements(state: State<'_, AppState>, kpi_id: String) -> Result<Vec<KpiMeasurementDto>, ApiError> {
    let kpi_id = parse_id(&kpi_id)?;

    ListKpiMeasurementsUseCase::new(state.repository.clone())
        .execute(kpi_id)
        .await
        .map(|measurements| measurements.into_iter().map(measurement_to_dto).collect())
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn list_all_kpi_measurements(state: State<'_, AppState>) -> Result<Vec<KpiMeasurementDto>, ApiError> {
    ListAllKpiMeasurementsUseCase::new(state.repository.clone())
        .execute()
        .await
        .map(|measurements| measurements.into_iter().map(measurement_to_dto).collect())
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn list_item_revisions(state: State<'_, AppState>, item_id: String) -> Result<Vec<ItemRevisionDto>, ApiError> {
    let item_id = parse_id(&item_id)?;

    ListItemRevisionsUseCase::new(state.repository.clone())
        .execute(item_id)
        .await
        .map(|revisions| revisions.into_iter().map(revision_to_dto).collect())
        .map_err(domain_error_to_api)
}

#[tauri::command]
pub async fn delete_kpi_measurement(state: State<'_, AppState>, kpi_id: String, measurement_id: String) -> Result<(), ApiError> {
    let kpi_id = parse_id(&kpi_id)?;
    let measurement_id = parse_id(&measurement_id)?;

    DeleteKpiMeasurementUseCase::new(state.repository.clone())
        .execute(kpi_id, measurement_id)
        .await
        .map_err(domain_error_to_api)
}
