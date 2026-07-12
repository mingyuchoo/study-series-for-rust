use crate::models::CreateAddressRequest;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[component]
pub fn AddressForm<F>(on_save: F) -> impl IntoView
where
    F: Fn() + 'static + Copy,
{
    let (name, set_name) = signal(String::new());
    let (phone, set_phone) = signal(String::new());
    let (email, set_email) = signal(String::new());
    let (is_saving, set_is_saving) = signal(false);

    let submit_form = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();

        if is_saving.get() {
            return;
        }

        set_is_saving.set(true);

        let request = CreateAddressRequest {
            name: name.get(),
            phone: phone.get(),
            email: email.get(),
        };

        spawn_local(async move {
            // Create a wrapper struct for the Tauri command arguments
            #[derive(serde::Serialize)]
            struct CreateAddressArgs {
                request: CreateAddressRequest,
            }

            let args = CreateAddressArgs {
                request,
            };
            match serde_wasm_bindgen::to_value(&args) {
                | Ok(args) => {
                    let result = invoke("create_address", args).await;
                    // Debug: log the raw result
                    web_sys::console::log_1(&format!("Raw result: {:?}", result).into());

                    // Try direct deserialization first
                    match serde_wasm_bindgen::from_value::<crate::models::AddressResponse>(result.clone()) {
                        | Ok(_address) => {
                            web_sys::console::log_1(&"Direct deserialization worked for create!".into());
                            // Clear form
                            set_name.set(String::new());
                            set_phone.set(String::new());
                            set_email.set(String::new());
                            on_save();
                            set_is_saving.set(false);
                            return;
                        },
                        | Err(err) => {
                            web_sys::console::log_1(&format!("Direct deserialization failed: {:?}", err).into());
                        },
                    }

                    // Fallback to Result wrapper
                    match serde_wasm_bindgen::from_value::<Result<crate::models::AddressResponse, String>>(result) {
                        | Ok(Ok(_)) => {
                            // Clear form
                            set_name.set(String::new());
                            set_phone.set(String::new());
                            set_email.set(String::new());
                            on_save();
                        },
                        | Ok(Err(err)) => {
                            web_sys::console::error_1(&format!("Error: {}", err).into());
                        },
                        | Err(err) => {
                            web_sys::console::error_1(&format!("Parse error: {:?}", err).into());
                        },
                    }
                },
                | Err(err) => {
                    web_sys::console::error_1(&format!("Serialization error: {:?}", err).into());
                },
            }
            set_is_saving.set(false);
        });
    };

    view! {
        <div class="address-form">
            <h2>"새 주소 추가"</h2>
            <form on:submit=submit_form>
                <div class="form-group">
                    <label>"이름:"</label>
                    <input
                        type="text"
                        prop:value=name
                        on:input=move |ev| set_name.set(event_target_value(&ev))
                        required
                    />
                </div>

                <div class="form-group">
                    <label>"전화번호:"</label>
                    <input
                        type="tel"
                        prop:value=phone
                        on:input=move |ev| set_phone.set(event_target_value(&ev))
                        required
                    />
                </div>

                <div class="form-group">
                    <label>"이메일:"</label>
                    <input
                        type="email"
                        prop:value=email
                        on:input=move |ev| set_email.set(event_target_value(&ev))
                        required
                    />
                </div>

                <button
                    type="submit"
                    class="btn btn-primary"
                    disabled=is_saving
                >
                    {move || if is_saving.get() { "저장 중..." } else { "저장" }}
                </button>
            </form>
        </div>
    }
}
