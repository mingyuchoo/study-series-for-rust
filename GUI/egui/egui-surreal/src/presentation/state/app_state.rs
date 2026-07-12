use crate::{domain::AppMessage,
            presentation::ui::AppTab};

pub struct AppState {
    // UI State
    pub current_tab: AppTab,

    // Person Management
    pub person_name: String,
    pub person_id_to_delete: String,
    pub people_list: String,

    // Authentication
    pub auth_username: String,
    pub auth_password: String,
    pub current_user: String,

    // Query
    pub raw_query: String,
    pub query_result: String,

    // Session
    pub session_info: String,

    // Messages and Status
    pub messages: Vec<AppMessage>,
    pub is_loading: bool,
    pub connection_status: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_tab: AppTab::People,
            person_name: String::new(),
            person_id_to_delete: String::new(),
            people_list: String::new(),
            auth_username: String::new(),
            auth_password: String::new(),
            current_user: String::new(),
            raw_query: String::new(),
            query_result: String::new(),
            session_info: String::new(),
            messages: Vec::new(),
            is_loading: false,
            connection_status: "Connected to localhost:8000".to_string(),
        }
    }
}
