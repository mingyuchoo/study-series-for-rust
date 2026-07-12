use crate::models::{Address,
                    UpdateAddressRequest};
use leptos::prelude::*;
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[component]
pub fn AddressItem<F, E>(address: Address, on_delete: F, on_edit: E) -> impl IntoView
where
    F: Fn(Uuid) + 'static + Copy + Send + Sync,
    E: Fn(Address) + 'static + Copy + Send + Sync,
{
    let (is_deleting, set_is_deleting) = signal(false);
    let (is_editing, set_is_editing) = signal(false);
    let (is_saving, set_is_saving) = signal(false);

    // Store address data in signals for editing
    let (name, set_name) = signal(address.name.clone());
    let (phone, set_phone) = signal(address.phone.clone());
    let (email, set_email) = signal(address.email.clone());

    // Editable fields
    let (edit_name, set_edit_name) = signal(address.name.clone());
    let (edit_phone, set_edit_phone) = signal(address.phone.clone());
    let (edit_email, set_edit_email) = signal(address.email.clone());

    let delete_address = move |_| {
        if is_deleting.get() {
            return;
        }

        set_is_deleting.set(true);
        let id = address.id;

        spawn_local(async move {
            // Create a wrapper struct for the Tauri command arguments
            #[derive(serde::Serialize)]
            struct DeleteAddressArgs {
                id: String,
            }

            let args = DeleteAddressArgs {
                id: id.to_string(),
            };
            let args = serde_wasm_bindgen::to_value(&args).unwrap();
            let result = invoke("delete_address", args).await;

            // Try direct deserialization first (Tauri unwraps Result automatically)
            match serde_wasm_bindgen::from_value::<bool>(result.clone()) {
                | Ok(true) => {
                    // Reset before triggering deletion callback to avoid setting a disposed signal
                    set_is_deleting.set(false);
                    on_delete(id);
                    return;
                },
                | Ok(false) => {
                    web_sys::console::error_1(&"Failed to delete address".into());
                    set_is_deleting.set(false);
                    return;
                },
                | Err(err) => {
                    web_sys::console::log_1(&format!("Direct bool deserialization failed: {:?}", err).into());
                },
            }

            // Fallback to Result wrapper
            match serde_wasm_bindgen::from_value::<Result<bool, String>>(result) {
                | Ok(Ok(true)) => {
                    // Reset before triggering deletion callback to avoid setting a disposed signal
                    set_is_deleting.set(false);
                    on_delete(id);
                    return;
                },
                | Ok(Ok(false)) => {
                    web_sys::console::error_1(&"Failed to delete address".into());
                },
                | Ok(Err(err)) => {
                    web_sys::console::error_1(&format!("Error: {}", err).into());
                },
                | Err(err) => {
                    web_sys::console::error_1(&format!("Parse error: {:?}", err).into());
                },
            }
            set_is_deleting.set(false);
        });
    };

    let start_edit = move |_| {
        set_edit_name.set(name.get());
        set_edit_phone.set(phone.get());
        set_edit_email.set(email.get());
        set_is_editing.set(true);
    };

    let cancel_edit = move |_| {
        set_is_editing.set(false);
    };

    let save_edit = move |_| {
        if is_saving.get() {
            return;
        }

        set_is_saving.set(true);
        let id = address.id;
        let name = edit_name.get();
        let phone = edit_phone.get();
        let email = edit_email.get();

        spawn_local(async move {
            #[derive(serde::Serialize)]
            struct UpdateAddressArgs {
                request: UpdateAddressRequest,
            }

            let request = UpdateAddressRequest {
                id,
                name: name.clone(),
                phone: phone.clone(),
                email: email.clone(),
            };

            let args = UpdateAddressArgs {
                request,
            };
            let args = serde_wasm_bindgen::to_value(&args).unwrap();
            let result = invoke("update_address", args).await;

            // Try direct deserialization as AddressResponse first
            match serde_wasm_bindgen::from_value::<crate::models::AddressResponse>(result.clone()) {
                | Ok(address_response) => {
                    let updated_address = Address::from(address_response);
                    // Update local signals
                    set_name.set(updated_address.name.clone());
                    set_phone.set(updated_address.phone.clone());
                    set_email.set(updated_address.email.clone());
                    set_is_saving.set(false);
                    set_is_editing.set(false);
                    on_edit(updated_address);
                    return;
                },
                | Err(err) => {
                    web_sys::console::log_1(&format!("Direct AddressResponse deserialization failed: {:?}", err).into());
                },
            }

            // Fallback to Result wrapper
            match serde_wasm_bindgen::from_value::<Result<crate::models::AddressResponse, String>>(result) {
                | Ok(Ok(address_response)) => {
                    let updated_address = Address::from(address_response);
                    // Update local signals
                    set_name.set(updated_address.name.clone());
                    set_phone.set(updated_address.phone.clone());
                    set_email.set(updated_address.email.clone());
                    set_is_saving.set(false);
                    set_is_editing.set(false);
                    on_edit(updated_address);
                    return;
                },
                | Ok(Err(err)) => {
                    web_sys::console::error_1(&format!("Error: {}", err).into());
                },
                | Err(err) => {
                    web_sys::console::error_1(&format!("Parse error: {:?}", err).into());
                },
            }
            set_is_saving.set(false);
        });
    };

    view! {
        <div class="address-item">
            <Show when=move || is_editing.get()>
                <div class="address-info editing">
                    <div class="form-group">
                        <label>"이름:"</label>
                        <input
                            type="text"
                            class="form-control"
                            prop:value=edit_name
                            on:input=move |ev| {
                                set_edit_name.set(event_target_value(&ev));
                            }
                        />
                    </div>
                    <div class="form-group">
                        <label>"전화:"</label>
                        <input
                            type="text"
                            class="form-control"
                            prop:value=edit_phone
                            on:input=move |ev| {
                                set_edit_phone.set(event_target_value(&ev));
                            }
                        />
                    </div>
                    <div class="form-group">
                        <label>"이메일:"</label>
                        <input
                            type="email"
                            class="form-control"
                            prop:value=edit_email
                            on:input=move |ev| {
                                set_edit_email.set(event_target_value(&ev));
                            }
                        />
                    </div>
                </div>
                <div class="address-actions">
                    <button
                        class="btn btn-success"
                        on:click=save_edit
                        disabled=is_saving
                    >
                        {move || if is_saving.get() { "저장 중..." } else { "저장" }}
                    </button>
                    <button
                        class="btn btn-secondary"
                        on:click=cancel_edit
                        disabled=is_saving
                    >
                        "취소"
                    </button>
                </div>
            </Show>

            <Show when=move || !is_editing.get()>
                <div class="address-info">
                    <h3>{name}</h3>
                    <p><strong>"전화:"</strong> {phone}</p>
                    <p><strong>"이메일:"</strong> {email}</p>
                </div>
                <div class="address-actions">
                    <button
                        class="btn btn-primary"
                        on:click=start_edit
                    >
                        "편집"
                    </button>
                    <button
                        class="btn btn-danger"
                        on:click=delete_address
                        disabled=is_deleting
                    >
                        {move || if is_deleting.get() { "삭제 중..." } else { "삭제" }}
                    </button>
                </div>
            </Show>
        </div>
    }
}
