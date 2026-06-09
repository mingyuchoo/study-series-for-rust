use crate::models::*;
use application::*;
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

pub struct AppState {
    pub create_contact_use_case: Arc<CreateContactUseCase>,
    pub get_contact_use_case: Arc<GetContactUseCase>,
    pub list_contacts_use_case: Arc<ListContactsUseCase>,
    pub update_contact_use_case: Arc<UpdateContactUseCase>,
    pub delete_contact_use_case: Arc<DeleteContactUseCase>,
    pub search_contacts_use_case: Arc<SearchContactsUseCase>,
}

fn parse_id(id: &str) -> Result<Uuid, ApiError> { Uuid::parse_str(id).map_err(|e| ApiError::invalid_id(format!("Invalid contact id: {e}"))) }

#[tauri::command]
pub async fn create_contact(state: State<'_, AppState>, request: CreateContactRequest) -> Result<ContactDto, ApiError> {
    state
        .create_contact_use_case
        .execute(request.name, request.email, request.phone, request.address)
        .await
        .map(ContactDto::from)
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn get_contact(state: State<'_, AppState>, id: String) -> Result<ContactDto, ApiError> {
    let uuid = parse_id(&id)?;

    state.get_contact_use_case.execute(uuid).await.map(ContactDto::from).map_err(ApiError::from)
}

#[tauri::command]
pub async fn list_contacts(state: State<'_, AppState>) -> Result<Vec<ContactDto>, ApiError> {
    state
        .list_contacts_use_case
        .execute()
        .await
        .map(|contacts| contacts.into_iter().map(ContactDto::from).collect())
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn update_contact(state: State<'_, AppState>, request: UpdateContactRequest) -> Result<ContactDto, ApiError> {
    let uuid = parse_id(&request.id)?;

    state
        .update_contact_use_case
        .execute(uuid, request.name, request.email, request.phone, request.address)
        .await
        .map(ContactDto::from)
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn delete_contact(state: State<'_, AppState>, id: String) -> Result<(), ApiError> {
    let uuid = parse_id(&id)?;

    state.delete_contact_use_case.execute(uuid).await.map_err(ApiError::from)
}

#[tauri::command]
pub async fn search_contacts(state: State<'_, AppState>, query: String) -> Result<Vec<ContactDto>, ApiError> {
    state
        .search_contacts_use_case
        .execute(&query)
        .await
        .map(|contacts| contacts.into_iter().map(ContactDto::from).collect())
        .map_err(ApiError::from)
}
