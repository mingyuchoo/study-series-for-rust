use crate::application::services::todo_application_service::TodoApplicationService;
use crate::domain::services::repositories::entities::todo::{Todo, TodoForm};
use crate::domain::services::todo_service::TodoService;
use crate::infrastructure::api::jsonplaceholder_api_controller::TodoApiController;
use crate::infrastructure::api::repositories::jsonplaceholder_api_repository::JsonPlaceholderTodoRepository;
use dioxus::prelude::*;

#[component]
pub fn TodosTab() -> Element {
    let mut todos = use_signal(Vec::<Todo>::new);
    let mut selected_todo = use_signal(|| None::<Todo>);
    let mut form = use_signal(TodoForm::default);
    let mut is_editing = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    // Load todos on component mount
    use_effect(move || {
        spawn(async move {
            match {
                let repo = JsonPlaceholderTodoRepository::new();
                let service = TodoService::new(repo);
                let app_service = TodoApplicationService::new(service);
                TodoApiController::new(app_service)
            }
            .find_all()
            .await
            {
                | Ok(fetched_todos) => {
                    // Limit to first 20 todos for better performance
                    todos.set(fetched_todos.into_iter().take(20).collect());
                },
                | Err(err) => {
                    error.set(Some(format!("Error loading todos: {}", err)));
                },
            }
        });

        // Return empty cleanup function
    });

    let handle_create = move |_| {
        let form_data = form();
        let mut form_clone = form;
        let mut todos_clone = todos;
        let mut error_clone = error;

        spawn(async move {
            match {
                let repo = JsonPlaceholderTodoRepository::new();
                let service = TodoService::new(repo);
                let app_service = TodoApplicationService::new(service);
                TodoApiController::new(app_service)
            }
            .create(form_data)
            .await
            {
                | Ok(new_todo) => {
                    todos_clone.write().push(new_todo.clone());
                    form_clone.set(TodoForm::default());
                    error_clone.set(None);
                },
                | Err(err) => {
                    error_clone.set(Some(format!("Error creating todo: {}", err)));
                },
            }
        });
    };

    let handle_update = move |_| {
        if let Some(todo) = selected_todo() {
            let form_data = form();
            let mut form_clone = form;
            let mut todos_clone = todos;
            let mut selected_todo_clone = selected_todo;
            let mut is_editing_clone = is_editing;
            let mut error_clone = error;

            spawn(async move {
                match {
                    let repo = JsonPlaceholderTodoRepository::new();
                    let service = TodoService::new(repo);
                    let app_service = TodoApplicationService::new(service);
                    TodoApiController::new(app_service)
                }
                .update(todo.id, form_data)
                .await
                {
                    | Ok(updated_todo) => {
                        let mut todos_write = todos_clone.write();
                        if let Some(index) = todos_write.iter().position(|item| item.id == updated_todo.id) {
                            todos_write[index] = updated_todo.clone();
                        }
                        selected_todo_clone.set(None);
                        form_clone.set(TodoForm::default());
                        is_editing_clone.set(false);
                        error_clone.set(None);
                    },
                    | Err(err) => {
                        error_clone.set(Some(format!("Error updating todo: {}", err)));
                    },
                }
            });
        }
    };

    let handle_delete = move |id: i32| {
        let mut todos_clone = todos;
        let mut selected_todo_clone = selected_todo;
        let mut form_clone = form;
        let mut is_editing_clone = is_editing;
        let mut error_clone = error;

        spawn(async move {
            match {
                let repo = JsonPlaceholderTodoRepository::new();
                let service = TodoService::new(repo);
                let app_service = TodoApplicationService::new(service);
                TodoApiController::new(app_service)
            }
            .delete(id)
            .await
            {
                | Ok(_) => {
                    todos_clone.write().retain(|todo| todo.id != id);
                    if selected_todo_clone().is_some_and(|t| t.id == id) {
                        selected_todo_clone.set(None);
                        form_clone.set(TodoForm::default());
                        is_editing_clone.set(false);
                    }
                    error_clone.set(None);
                },
                | Err(err) => {
                    error_clone.set(Some(format!("Error deleting todo: {}", err)));
                },
            }
        });
    };

    let mut handle_edit = move |todo: Todo| {
        selected_todo.set(Some(todo.clone()));
        form.set(TodoForm {
            userId: todo.userId,
            title: todo.title,
            completed: todo.completed,
        });
        is_editing.set(true);
    };

    let handle_cancel = move |_| {
        form.set(TodoForm::default());
        is_editing.set(false);
    };

    let toggle_completed = move |todo: Todo| {
        let mut todos_clone = todos;
        let mut error_clone = error;

        spawn(async move {
            let updated_form = TodoForm {
                userId: todo.userId,
                title: todo.title.clone(),
                completed: !todo.completed,
            };

            match {
                let repo = JsonPlaceholderTodoRepository::new();
                let service = TodoService::new(repo);
                let app_service = TodoApplicationService::new(service);
                TodoApiController::new(app_service)
            }
            .update(todo.id, updated_form)
            .await
            {
                | Ok(updated_todo) => {
                    let mut todos_write = todos_clone.write();
                    if let Some(index) = todos_write.iter().position(|item| item.id == updated_todo.id) {
                        todos_write[index] = updated_todo.clone();
                    }
                    error_clone.set(None);
                },
                | Err(err) => {
                    error_clone.set(Some(format!("Error updating todo: {}", err)));
                },
            }
        });
    };

    rsx! {
        div { class: "resource-page",
            div { class: "resource-header",
                h1 { class: "page-title", "Todos" }
                p { class: "page-kicker", "Edit task records and toggle completion state." }
            }

            // Error message
            {error().map(|err| rsx!(
                div { class: "notice",
                    p { {err} }
                }
            ))}

            // Todo form
            div { class: "panel form-panel",
                h2 { class: "panel-title",
                    {if is_editing() { "Edit todo" } else { "Add new todo" }}
                }
                div { class: "form-grid",
                    div { class: "field",
                        label { "User ID" }
                        input {
                            type: "number",
                            value: form().userId.to_string(),
                            oninput: move |evt| {
                                let mut form_write = form.write();
                                if let Ok(id) = evt.value().parse::<i32>() {
                                    form_write.userId = id;
                                }
                            }
                        }
                    }
                    div { class: "field",
                        label { "Title" }
                        input {
                            type: "text",
                            value: form().title.clone(),
                            oninput: move |evt| {
                                let mut form_write = form.write();
                                form_write.title = evt.value().clone();
                            }
                        }
                    }
                    div { class: "checkbox-field",
                        input {
                            id: "completed",
                            type: "checkbox",
                            checked: form().completed,
                            oninput: move |evt| {
                                let mut form_write = form.write();
                                form_write.completed = evt.value().parse().unwrap_or(false);
                            }
                        }
                        label { r#for: "completed", "Completed" }
                    }
                }
                div { class: "form-actions",
                    {if is_editing() {
                        rsx! {
                            button {
                                type: "submit",
                                onclick: handle_update,
                                "Update Todo"
                            }
                            button {
                                type: "button",
                                onclick: handle_cancel,
                                "Cancel"
                            }
                        }
                    } else {
                        rsx! {
                            button {
                                type: "submit",
                                onclick: handle_create,
                                "Add Todo"
                            }
                        }
                    }}
                }
            }

            // Todos list
            div { class: "table-panel",
                div { class: "table-caption",
                    h3 { "Todos" }
                    span { class: "command-tag", "{todos().len()} rows" }
                }
                table {
                    thead {
                        tr {
                            th {  "ID" }
                            th {  "User ID" }
                            th {  "Title" }
                            th {  "Status" }
                            th {  "Actions" }
                        }
                    }
                    tbody {
                        {todos().into_iter().map(|todo| {
                            let todo_id = todo.id;
                            let todo_for_toggle = todo.clone();
                            let todo_for_edit = todo.clone();
                            rsx!(
                                tr { key: "{todo.id}",
                                    td {  {todo.id.to_string()} }
                                    td {  {todo.userId.to_string()} }
                                    td {  {todo.title.clone()} }
                                    td {
                                        div { class: "status-control",
                                            input {
                                                type: "checkbox",
                                                checked: todo.completed,
                                                onclick: move |_| toggle_completed(todo_for_toggle.clone())
                                            }
                                            span { class: "status-pill",
                                                {if todo.completed { "Completed" } else { "Pending" }}
                                            }
                                        }
                                    }
                                    td {
                                        div { class: "row-actions",
                                            button {
                                                type: "button",
                                                onclick: move |_| handle_edit(todo_for_edit.clone()),
                                                "Edit"
                                            }
                                            button {
                                                type: "button",
                                                onclick: move |_| handle_delete(todo_id),
                                                "Delete"
                                            }
                                        }
                                    }
                                }
                            )
                        })}
                    }
                }
            }
        }
    }
}
