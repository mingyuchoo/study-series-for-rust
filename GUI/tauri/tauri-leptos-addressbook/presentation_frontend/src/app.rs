use crate::components::{address_form::AddressForm,
                        address_list::AddressList};
use leptos::prelude::*;

#[component]
pub fn App() -> impl IntoView {
    let (show_form, set_show_form) = signal(false);
    let (refresh_trigger, set_refresh_trigger) = signal(0);

    let toggle_form = move |_| {
        set_show_form.update(|show| *show = !*show);
    };

    let on_address_saved = move || {
        set_show_form.set(false);
        set_refresh_trigger.update(|n| *n += 1);
    };

    view! {
        <div class="container">
            <header class="header">
                <h1>"주소록 앱"</h1>
                <button
                    class="btn btn-primary"
                    on:click=toggle_form
                >
                    {move || if show_form.get() { "취소" } else { "새 주소 추가" }}
                </button>
            </header>

            <main class="main">
                <Show when=move || show_form.get()>
                    <AddressForm on_save=on_address_saved />
                </Show>

                <AddressList refresh_trigger=refresh_trigger />
            </main>
        </div>
    }
}
