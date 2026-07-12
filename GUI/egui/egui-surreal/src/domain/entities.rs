use serde::{Deserialize,
            Serialize};
use surrealdb::RecordId;

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct PersonData {
    pub name: String,
    pub id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Person {
    pub name: String,
    pub id: RecordId,
    pub created_by: Option<RecordId>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthParams {
    pub name: String,
    pub pass: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct AppMessage {
    pub content: String,
    pub msg_type: MessageType,
    pub timestamp: std::time::Instant,
}
