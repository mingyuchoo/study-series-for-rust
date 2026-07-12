use crate::{components::address_item::AddressItem,
            models::Address};
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[component]
pub fn AddressList(refresh_trigger: ReadSignal<i32>) -> impl IntoView {
    let (addresses, set_addresses) = signal(Vec::<Address>::new());
    let (loading, set_loading) = signal(true);

    let load_addresses = move || {
        set_loading.set(true);
        spawn_local(async move {
            // First test the simple command
            let test_result = invoke("test_command", JsValue::NULL).await;
            web_sys::console::log_1(&format!("Test result: {:?}", test_result).into());

            // Test simple addresses without UUIDs
            let simple_result = invoke("get_simple_addresses", JsValue::NULL).await;
            web_sys::console::log_1(&format!("Simple result: {:?}", simple_result).into());

            let result = invoke("get_all_addresses", JsValue::NULL).await;
            // Debug: log the raw result
            web_sys::console::log_1(&format!("Raw result: {:?}", result).into());

            // Try to deserialize as a generic value first
            match js_sys::JSON::stringify(&result) {
                | Ok(json_str) => {
                    web_sys::console::log_1(&format!("JSON result: {}", json_str).into());
                },
                | Err(_) => {
                    web_sys::console::log_1(&"Failed to stringify result".into());
                },
            }

            // Try deserializing directly as Vec<AddressResponse> first
            match serde_wasm_bindgen::from_value::<Vec<crate::models::AddressResponse>>(result.clone()) {
                | Ok(addr_list) => {
                    web_sys::console::log_1(&"Direct deserialization worked!".into());
                    set_addresses.set(addr_list);
                    set_loading.set(false);
                    return;
                },
                | Err(err) => {
                    web_sys::console::log_1(&format!("Direct deserialization failed: {:?}", err).into());
                },
            }

            // Fallback to Result wrapper
            match serde_wasm_bindgen::from_value::<Result<Vec<crate::models::AddressResponse>, String>>(result) {
                | Ok(Ok(addr_list)) => {
                    set_addresses.set(addr_list);
                },
                | Ok(Err(err)) => {
                    web_sys::console::error_1(&format!("Error loading addresses: {}", err).into());
                },
                | Err(err) => {
                    web_sys::console::error_1(&format!("Parse error: {:?}", err).into());
                },
            }
            set_loading.set(false);
        });
    };

    // Load addresses on mount and when refresh_trigger changes
    Effect::new(move |_| {
        refresh_trigger.track();
        load_addresses();
    });

    let on_delete = move |_id: uuid::Uuid| {
        load_addresses();
    };

    let on_edit = move |_updated_address: Address| {
        load_addresses();
    };

    view! {
        <div class="address-list">
            <h2>"주소 목록"</h2>

            <Show when=move || loading.get()>
                <div class="loading">"로딩 중..."</div>
            </Show>

            <Show when=move || !loading.get()>
                <div class="addresses">
                    <For
                        each=move || addresses.get()
                        key=|address| address.id
                        children=move |address| {
                            view! {
                                <AddressItem
                                    address=address
                                    on_delete=on_delete
                                    on_edit=on_edit
                                />
                            }
                        }
                    />

                    <Show when=move || addresses.get().is_empty()>
                        <div class="empty-state">
                            "등록된 주소가 없습니다."
                        </div>
                    </Show>
                </div>
            </Show>
        </div>
    }
}
