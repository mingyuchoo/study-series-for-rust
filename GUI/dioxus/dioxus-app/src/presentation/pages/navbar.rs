use crate::presentation::Route;
use dioxus::prelude::*;

/// Shared navbar component.
#[component]
pub fn Navbar() -> Element {
    rsx! {
        header { class: "site-header",
            Link { class: "brand", to: Route::Home {},
                span { class: "brand-mark", "API" }
                span { "JSONPlaceholder Manager" }
            }
            nav { class: "nav-links",
                Link { class: "nav-link", to: Route::Users {}, "Users" }
                Link { class: "nav-link", to: Route::Todos {}, "Todos" }
                Link { class: "nav-link", to: Route::Posts {}, "Posts" }
                Link { class: "nav-link", to: Route::Docs {}, "Documents" }
            }
        }
        main { class: "app-content",
            Outlet::<Route> {}
        }
    }
}
