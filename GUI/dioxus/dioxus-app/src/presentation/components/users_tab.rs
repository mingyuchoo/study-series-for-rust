use crate::application::services::user_application_service::UserApplicationService;
use crate::domain::services::repositories::entities::user::{User, UserForm};
use crate::domain::services::user_service::UserService;
use crate::infrastructure::api::jsonplaceholder_api_controller::UserApiController;
use crate::infrastructure::api::repositories::jsonplaceholder_api_repository::JsonPlaceholderUserRepository;
use dioxus::prelude::*;

#[component]
pub fn UsersTab() -> Element {
    let mut users = use_signal(Vec::<User>::new);
    let mut selected_user = use_signal(|| None::<User>);
    let mut form = use_signal(UserForm::default);
    let mut is_editing = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    // Load users on component mount
    use_effect(move || {
        spawn(async move {
            match {
                let repo = JsonPlaceholderUserRepository::new();
                let service = UserService::new(repo);
                let app_service = UserApplicationService::new(service);
                UserApiController::new(app_service)
            }
            .find_all()
            .await
            {
                | Ok(fetched_users) => {
                    users.set(fetched_users);
                },
                | Err(err) => {
                    error.set(Some(format!("Error loading users: {}", err)));
                },
            }
        });

        // Return empty cleanup function
    });

    let handle_create = move |_| {
        let form_data = form();
        let mut form_clone = form;
        let mut users_clone = users;
        let mut error_clone = error;

        spawn(async move {
            match {
                let repo = JsonPlaceholderUserRepository::new();
                let service = UserService::new(repo);
                let app_service = UserApplicationService::new(service);
                UserApiController::new(app_service)
            }
            .create(form_data)
            .await
            {
                | Ok(new_user) => {
                    users_clone.write().push(new_user.clone());
                    form_clone.set(UserForm::default());
                    error_clone.set(None);
                },
                | Err(err) => {
                    error_clone.set(Some(format!("Error creating user: {}", err)));
                },
            }
        });
    };

    let handle_update = move |_| {
        if let Some(user) = selected_user() {
            let form_data = form();
            let mut users_clone = users;
            let mut selected_user_clone = selected_user;
            let mut form_clone = form;
            let mut is_editing_clone = is_editing;
            let mut error_clone = error;

            spawn(async move {
                match {
                    let repo = JsonPlaceholderUserRepository::new();
                    let service = UserService::new(repo);
                    let app_service = UserApplicationService::new(service);
                    UserApiController::new(app_service)
                }
                .update(user.id, form_data)
                .await
                {
                    | Ok(updated_user) => {
                        let mut users_write = users_clone.write();
                        if let Some(index) = users_write.iter().position(|item| item.id == updated_user.id) {
                            users_write[index] = updated_user.clone();
                        }
                        selected_user_clone.set(None);
                        form_clone.set(UserForm::default());
                        is_editing_clone.set(false);
                        error_clone.set(None);
                    },
                    | Err(err) => {
                        error_clone.set(Some(format!("Error updating user: {}", err)));
                    },
                }
            });
        }
    };

    let handle_delete = move |id: i32| {
        let mut users_clone = users;
        let mut selected_user_clone = selected_user;
        let mut form_clone = form;
        let mut is_editing_clone = is_editing;
        let mut error_clone = error;

        spawn(async move {
            match {
                let repo = JsonPlaceholderUserRepository::new();
                let service = UserService::new(repo);
                let app_service = UserApplicationService::new(service);
                UserApiController::new(app_service)
            }
            .delete(id)
            .await
            {
                | Ok(_) => {
                    users_clone.write().retain(|user| user.id != id);
                    if selected_user_clone().is_some_and(|u| u.id == id) {
                        selected_user_clone.set(None);
                        form_clone.set(UserForm::default());
                        is_editing_clone.set(false);
                    }
                    error_clone.set(None);
                },
                | Err(err) => {
                    error_clone.set(Some(format!("Error deleting user: {}", err)));
                },
            }
        });
    };

    let mut handle_edit = move |user: User| {
        selected_user.set(Some(user.clone()));
        form.set(UserForm {
            name: user.name,
            username: user.username,
            email: user.email,
            phone: user.phone.unwrap_or_default(),
        });
        is_editing.set(true);
    };

    let handle_cancel = move |_| {
        form.set(UserForm::default());
        is_editing.set(false);
    };

    rsx! {
        div { class: "resource-page",
            div { class: "resource-header",
                h1 { class: "page-title", "Users" }
                p { class: "page-kicker", "Manage identity and contact data from JSONPlaceholder." }
            }

            // Error message
            {error().map(|err| rsx!(
                div { class: "notice",
                    p { {err} }
                }
            ))}

            // User form
            div { class: "panel form-panel",
                h2 { class: "panel-title",
                    {if is_editing() { "Edit user" } else { "Add new user" }}
                }

                div { class: "form-grid",
                    div { class: "field",
                        label { "Name" }
                        input {
                            type: "text",
                            value: form().name.clone(),
                            oninput: move |evt| {
                                let mut form_write = form.write();
                                form_write.name = evt.value().clone();
                            }
                        }
                    }

                    div { class: "field",
                        label { "Username" }
                        input {
                            type: "text",
                            value: form().username.clone(),
                            oninput: move |evt| {
                                let mut form_write = form.write();
                                form_write.username = evt.value().clone();
                            }
                        }
                    }

                    div { class: "field",
                        label { "Email" }
                        input {
                            type: "email",
                            value: form().email.clone(),
                            oninput: move |evt| {
                                let mut form_write = form.write();
                                form_write.email = evt.value().clone();
                            }
                        }
                    }

                    div { class: "field",
                        label { "Phone" }
                        input {
                            type: "text",
                            value: form().phone.clone(),
                            oninput: move |evt| {
                                let mut form_write = form.write();
                                form_write.phone = evt.value().clone();
                            }
                        }
                    }
                }

                div { class: "form-actions",
                    {if is_editing() {
                        rsx! {
                            button {
                                type: "submit",
                                onclick: handle_update,
                                "Update User"
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
                                "Add User"
                            }
                        }
                    }}
                }
            }

            // Users list
            div { class: "table-panel",
                div { class: "table-caption",
                    h3 { "Users" }
                    span { class: "command-tag", "{users().len()} rows" }
                }
                table {
                    thead {
                        tr {
                            th {  "ID" }
                            th {  "Name" }
                            th {  "Username" }
                            th {  "Email" }
                            th {  "Actions" }
                        }
                    }
                    tbody {
                        {users().into_iter().map(|user| {
                            let user_clone = user.clone();
                            let user_id = user.id;
                            rsx!(
                                tr { key: "{user.id}",
                                    td {  {user.id.to_string()} }
                                    td {  {user.name.clone()} }
                                    td {  {user.username.clone()} }
                                    td {  {user.email.clone()} }
                                    td {
                                        div { class: "row-actions",
                                            button {

                                                onclick: move |_| handle_edit(user_clone.clone()),
                                                "Edit"
                                            }
                                            button {

                                                onclick: move |_| handle_delete(user_id),
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
