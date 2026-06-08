use crate::presentation::Route;
use dioxus::prelude::*;

/// Home page
#[component]
pub fn Home() -> Element {
    rsx! {
        div { class: "home-page",
            section { class: "home-hero",
                div { class: "mascot", "API" }
                h1 { "A quiet workspace for sample API data" }
                p { "Users, todos, posts, and local documents on one paper-white canvas." }
                code { class: "install-snippet", "GET https://jsonplaceholder.typicode.com" }
                Link { class: "pill-link", to: Route::Users {}, "Open users" }
            }

            section { class: "home-section split",
                div {
                    h2 { "API data, shaped like documentation" }
                    p { class: "section-copy", "Flat tables, small controls, and local document notes keep the app close to the command-line tools it represents." }
                }
                div { class: "terminal-card",
                    div { class: "terminal-lights",
                        span { class: "terminal-red" }
                        span { class: "terminal-yellow" }
                        span { class: "terminal-green" }
                    }
                    p { class: "terminal-line", "$ cargo run" }
                    p { class: "terminal-line terminal-muted", "# fetch users, todos, posts" }
                    p { class: "terminal-line", "$ ./scripts/run.sh build test" }
                }
            }

            section { class: "home-section",
                h2 { "Sections" }
                div { class: "home-grid",
                    div { class: "home-card",
                        h3 { "Users" }
                        p { "Review account records and contact fields." }
                        Link { class: "pill-link", to: Route::Users {}, "Go to users" }
                    }
                    div { class: "home-card",
                        h3 { "Todos" }
                        p { "Track status and edit sample task data." }
                        Link { class: "pill-link", to: Route::Todos {}, "Go to todos" }
                    }
                    div { class: "home-card",
                        h3 { "Posts" }
                        p { "Create and update post records." }
                        Link { class: "pill-link", to: Route::Posts {}, "Go to posts" }
                    }
                    div { class: "home-card",
                        h3 { "Documents" }
                        p { "Keep local notes in the app database." }
                        Link { class: "pill-link", to: Route::Docs {}, "Go to documents" }
                    }
                }
            }
        }
    }
}
