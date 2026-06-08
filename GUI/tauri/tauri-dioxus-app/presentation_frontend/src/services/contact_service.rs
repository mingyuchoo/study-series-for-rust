use crate::models::{Contact,
                    CreateContactRequest,
                    UpdateContactRequest};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

pub struct ContactService;

impl ContactService {
    pub async fn create_contact(request: CreateContactRequest) -> Result<Contact, String> {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "request": request })).map_err(|e| format!("Serialization error: {}", e))?;

        let result = invoke("create_contact", args).await;

        serde_wasm_bindgen::from_value(result).map_err(|e| format!("Deserialization error: {}", e))
    }

    #[allow(dead_code)]
    pub async fn get_contact(id: String) -> Result<Contact, String> {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "id": id })).map_err(|e| format!("Serialization error: {}", e))?;

        let result = invoke("get_contact", args).await;

        serde_wasm_bindgen::from_value(result).map_err(|e| format!("Deserialization error: {}", e))
    }

    pub async fn list_contacts() -> Result<Vec<Contact>, String> {
        let result = invoke("list_contacts", JsValue::NULL).await;

        serde_wasm_bindgen::from_value(result).map_err(|e| format!("Deserialization error: {}", e))
    }

    pub async fn update_contact(request: UpdateContactRequest) -> Result<Contact, String> {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "request": request })).map_err(|e| format!("Serialization error: {}", e))?;

        let result = invoke("update_contact", args).await;

        serde_wasm_bindgen::from_value(result).map_err(|e| format!("Deserialization error: {}", e))
    }

    pub async fn delete_contact(id: String) -> Result<(), String> {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "id": id })).map_err(|e| format!("Serialization error: {}", e))?;

        let result = invoke("delete_contact", args).await;

        if result.is_null() || result.is_undefined() {
            Ok(())
        } else {
            serde_wasm_bindgen::from_value(result).map_err(|e| format!("Deserialization error: {}", e))
        }
    }

    pub async fn search_contacts(query: String) -> Result<Vec<Contact>, String> {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "query": query })).map_err(|e| format!("Serialization error: {}", e))?;

        let result = invoke("search_contacts", args).await;

        serde_wasm_bindgen::from_value(result).map_err(|e| format!("Deserialization error: {}", e))
    }
}
