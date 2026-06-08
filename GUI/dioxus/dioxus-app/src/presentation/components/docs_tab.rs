use crate::application::services::doc_application_service::DocApplicationService;
use crate::domain::services::doc_service::DocService;
use crate::domain::services::repositories::entities::doc::DocForm;
use crate::infrastructure::db::repositories::doc_db_repository::DocDbRepository;
use dioxus::prelude::*;
use std::path::Path;

#[component]
pub fn DocsTab() -> Element {
    // State for the document form
    let mut title = use_signal(String::new);
    let mut contents = use_signal(String::new);

    // State for the document list
    let mut documents = use_signal(Vec::new);
    let mut error = use_signal(|| Option::<String>::None);

    // State for editing
    let mut editing_id = use_signal(|| Option::<String>::None);

    // Initialize the repository and service
    // Create a proper absolute path for the database in the user's home directory
    let db_path = use_signal(|| match dirs::home_dir() {
        | Some(mut path) => {
            path.push(".local");
            path.push("share");
            path.push("dioxus-app");
            std::fs::create_dir_all(&path).unwrap_or_default();
            path.push("docs.db");
            path.to_str().unwrap_or("docs.db").to_string()
        },
        | None => "docs.db".to_string(),
    });

    // Function to load documents
    let mut load_documents = move || {
        let db_path_str = db_path();
        println!("Loading documents from database: {}", db_path_str);
        if let Ok(repo) = DocDbRepository::new(&db_path_str) {
            let doc_service = DocService::new(repo);
            let app_service = DocApplicationService::new(doc_service);

            match app_service.list_all_docs() {
                | Ok(docs) => {
                    documents.set(docs);
                    error.set(None);
                },
                | Err(err) => {
                    error.set(Some(format!("Error loading documents: {}", err)));
                },
            }
        } else {
            error.set(Some(format!("Failed to connect to database at {}", db_path)));
        }
    };

    // Load documents when component mounts
    use_effect(move || {
        load_documents();
        // Return empty cleanup function
        {}
    });

    // Handle form submission
    let handle_submit = move |event: FormEvent| {
        event.prevent_default();
        let db_path_str = db_path();
        println!("Submitting form to database: {}", db_path_str);

        if let Ok(repo) = DocDbRepository::new(&db_path_str) {
            let doc_service = DocService::new(repo);
            let app_service = DocApplicationService::new(doc_service);

            if let Some(id) = editing_id() {
                // Update existing document
                if let Some(doc) = app_service.get_doc_details(&id) {
                    let updated_form = DocForm {
                        title: title().clone(),
                        contents: contents().clone(),
                        archived: doc.archived,
                    };

                    if futures::executor::block_on(app_service.update(doc.id, updated_form)).is_ok() {
                        // Reset form
                        title.set(String::new());
                        contents.set(String::new());
                        editing_id.set(None);

                        // Reload documents
                        load_documents();
                    } else {
                        error.set(Some("Failed to update document".to_string()));
                    }
                }
            } else {
                // Create new document
                match app_service.register_doc(title().clone(), contents().clone()) {
                    | Ok(_) => {
                        // Reset form
                        title.set(String::new());
                        contents.set(String::new());

                        // Reload documents
                        load_documents();
                    },
                    | Err(err) => {
                        error.set(Some(format!("Error creating document: {}", err)));
                    },
                }
            }
        } else {
            error.set(Some(format!("Failed to connect to database at {}", db_path)));
        }
    };

    // Handle document deletion
    let mut handle_delete = move |id: String| {
        let db_path_str = db_path();
        if let Ok(repo) = DocDbRepository::new(&db_path_str) {
            let doc_service = DocService::new(repo);
            let app_service = DocApplicationService::new(doc_service);

            if app_service.delete_doc(&id).is_ok() {
                // Reload documents
                load_documents();
            } else {
                error.set(Some(format!("Failed to delete document with ID: {}", id)));
            }
        }
    };

    // Handle document editing
    let mut handle_edit = move |id: String| {
        let db_path_str = db_path();
        if let Ok(repo) = DocDbRepository::new(&db_path_str) {
            let doc_service = DocService::new(repo);
            let app_service = DocApplicationService::new(doc_service);

            if let Some(doc) = app_service.get_doc_details(&id) {
                // Set form values
                title.set(doc.title.clone());
                contents.set(doc.contents.clone());
                editing_id.set(Some(id));
            }
        }
    };

    // Handle form cancellation
    let handle_cancel = move |_| {
        title.set(String::new());
        contents.set(String::new());
        editing_id.set(None);
    };

    // Check if DB exists and show appropriate message
    let db_exists = Path::new(&db_path()).exists();

    rsx! {
        div { class: "resource-page",
            div { class: "resource-header",
                h1 { class: "page-title", "Documents" }
                p { class: "page-kicker", "Store local notes beside the remote sample data." }
            }

            // Error message
            {error().map(|err| rsx! {
                div { class: "notice",
                    p { "{err}" }
                }
            })}

            // DB status message
            {if !db_exists {
                rsx! {
                    div { class: "notice",
                        p { "Database file will be created automatically when you add your first document." }
                    }
                }
            } else { rsx!{} }}

            // Document form
            div { class: "panel form-panel",
                form { onsubmit: handle_submit,
                    h2 { class: "panel-title",
                        if editing_id().is_some() { "Edit document" } else { "Add new document" }
                    }

                    div { class: "form-grid",
                    div { class: "field",
                        label { "Title" }
                        input {
                            "type": "text",
                            placeholder: "Document title",
                            value: title,
                            oninput: move |evt| title.set(evt.value().clone()),
                            required: true
                        }
                    }

                    div { class: "field",
                        label { "Contents" }
                        textarea {
                            placeholder: "Document contents",
                            value: contents,
                            oninput: move |evt| contents.set(evt.value().clone()),
                            rows: 5,
                            required: true
                        }
                    }
                    }

                    div { class: "form-actions",
                        button {
                            "type": "submit",
                            if editing_id().is_some() { "Update Document" } else { "Add Document" }
                        }

                        if editing_id().is_some() {
                            button {
                                "type": "button",
                                onclick: handle_cancel,
                                "Cancel"
                            }
                        }
                    }
                }
            }

            // Document list
            div { class: "table-panel",
                div { class: "table-caption",
                    h3 { "Documents" }
                    span { class: "command-tag", "{documents().len()} rows" }
                }

                if documents().is_empty() {
                    p { class: "empty-state", "No documents found." }
                } else {
                    table {
                        thead {
                            tr {
                                th { "ID" }
                                th { "Title" }
                                th { "Status" }
                                th { "Actions" }
                            }
                        }
                        tbody {
                            for doc in documents.peek().iter().cloned() {
                                tr {
                                    td { "{doc.id}" }
                                    td { "{doc.title}" }
                                    td {
                                        if doc.archived {
                                            span { class: "status-pill", "Archived" }
                                        } else {
                                            span { class: "status-pill", "Active" }
                                        }
                                    }
                                    td {
                                        div { class: "row-actions",
                                            button {
                                                onclick: move |_| handle_edit(doc.id.to_string()),
                                                "Edit"
                                            }
                                            button {
                                                onclick: move |_| handle_delete(doc.id.to_string()),
                                                "Delete"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
