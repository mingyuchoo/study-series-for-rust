use crate::models::{ApiError,
                    CreateItemRequest,
                    KpiMeasurement,
                    RecordKpiMeasurementRequest,
                    UpdateItemRequest,
                    VvkikItem};
use js_sys::Reflect;
use serde::{Serialize,
            de::DeserializeOwned};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], catch)]
    async fn invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

pub struct VvkikService;

fn rejection_to_message(error: JsValue) -> String {
    if let Ok(api_error) = serde_wasm_bindgen::from_value::<ApiError>(error.clone()) {
        return api_error.message;
    }

    error
        .as_string()
        .or_else(|| js_sys::Error::from(error).message().as_string())
        .unwrap_or_else(|| "알 수 없는 오류가 발생했습니다.".to_string())
}

fn is_tauri_runtime_available() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };

    let Ok(tauri) = Reflect::get(&window, &JsValue::from_str("__TAURI__")) else {
        return false;
    };
    if tauri.is_null() || tauri.is_undefined() {
        return false;
    }

    let Ok(core) = Reflect::get(&tauri, &JsValue::from_str("core")) else {
        return false;
    };
    if core.is_null() || core.is_undefined() {
        return false;
    }

    let Ok(invoke) = Reflect::get(&core, &JsValue::from_str("invoke")) else {
        return false;
    };

    invoke.is_function()
}

fn require_tauri_runtime() -> Result<(), String> {
    is_tauri_runtime_available()
        .then_some(())
        .ok_or_else(|| "Tauri 런타임에서 실행해 주세요.".to_string())
}

/// 직렬화 → invoke → 오류 변환까지의 공통 경로.
async fn call_raw(cmd: &str, args: &impl Serialize) -> Result<JsValue, String> {
    require_tauri_runtime()?;

    let args = serde_wasm_bindgen::to_value(args).map_err(|e| format!("Serialization error: {}", e))?;
    invoke(cmd, args).await.map_err(rejection_to_message)
}

/// 응답 역직렬화까지 포함한 일반 커맨드 호출.
async fn call<R: DeserializeOwned>(cmd: &str, args: &impl Serialize) -> Result<R, String> {
    let result = call_raw(cmd, args).await?;
    serde_wasm_bindgen::from_value(result).map_err(|e| format!("Deserialization error: {}", e))
}

impl VvkikService {
    pub async fn create_item(request: CreateItemRequest) -> Result<VvkikItem, String> {
        call("create_item", &serde_json::json!({ "request": request })).await
    }

    pub async fn list_items() -> Result<Vec<VvkikItem>, String> {
        // 브라우저 단독 실행(개발 미리보기 등)에서는 빈 목록으로 동작한다.
        if !is_tauri_runtime_available() {
            return Ok(Vec::new());
        }

        call("list_items", &serde_json::json!({})).await
    }

    pub async fn update_item(request: UpdateItemRequest) -> Result<VvkikItem, String> {
        call("update_item", &serde_json::json!({ "request": request })).await
    }

    pub async fn delete_item(id: String) -> Result<(), String> {
        let result = call_raw("delete_item", &serde_json::json!({ "id": id })).await?;

        if result.is_null() || result.is_undefined() {
            Ok(())
        } else {
            serde_wasm_bindgen::from_value(result).map_err(|e| format!("Deserialization error: {}", e))
        }
    }

    pub async fn search_items(query: String) -> Result<Vec<VvkikItem>, String> {
        if !is_tauri_runtime_available() {
            return Ok(Vec::new());
        }

        call("search_items", &serde_json::json!({ "query": query })).await
    }

    #[allow(dead_code)]
    pub async fn record_kpi_measurement(request: RecordKpiMeasurementRequest) -> Result<KpiMeasurement, String> {
        call("record_kpi_measurement", &serde_json::json!({ "request": request })).await
    }
}
