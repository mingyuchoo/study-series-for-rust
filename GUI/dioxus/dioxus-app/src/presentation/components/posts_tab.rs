use crate::application::services::post_application_service::PostApplicationService;
use crate::domain::services::post_service::PostService;
use crate::domain::services::repositories::entities::post::{Post, PostForm};
use crate::infrastructure::api::jsonplaceholder_api_controller::PostApiController;
use crate::infrastructure::api::repositories::jsonplaceholder_api_repository::JsonPlaceholderPostRepository;
use dioxus::prelude::*;

#[component]
pub fn PostsTab() -> Element {
    let mut posts = use_signal(Vec::<Post>::new);
    let mut selected_post = use_signal(|| None::<Post>);
    let mut form = use_signal(PostForm::default);
    let mut is_editing = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    // Load posts on component mount
    use_effect(move || {
        spawn(async move {
            match {
                let repo = JsonPlaceholderPostRepository::new();
                let service = PostService::new(repo);
                let app_service = PostApplicationService::new(service);
                PostApiController::new(app_service)
            }
            .find_all()
            .await
            {
                | Ok(fetched_posts) => {
                    // Limit to first 20 posts for better performance
                    posts.set(fetched_posts.into_iter().take(20).collect());
                },
                | Err(err) => {
                    error.set(Some(format!("Error loading posts: {}", err)));
                },
            }
        });

        // Return empty cleanup function
    });

    let handle_create = move |_| {
        let form_data = form();
        let mut form_clone = form;
        let mut posts_clone = posts;
        let mut error_clone = error;

        spawn(async move {
            match {
                let repo = JsonPlaceholderPostRepository::new();
                let service = PostService::new(repo);
                let app_service = PostApplicationService::new(service);
                PostApiController::new(app_service)
            }
            .create(form_data)
            .await
            {
                | Ok(new_post) => {
                    posts_clone.write().push(new_post.clone());
                    form_clone.set(PostForm::default());
                    error_clone.set(None);
                },
                | Err(err) => {
                    error_clone.set(Some(format!("Error creating post: {}", err)));
                },
            }
        });
    };

    let handle_update = move |_| {
        if let Some(post) = selected_post() {
            let form_data = form();
            let mut form_clone = form;
            let mut posts_clone = posts;
            let mut selected_post_clone = selected_post;
            let mut is_editing_clone = is_editing;
            let mut error_clone = error;

            spawn(async move {
                match {
                    let repo = JsonPlaceholderPostRepository::new();
                    let service = PostService::new(repo);
                    let app_service = PostApplicationService::new(service);
                    PostApiController::new(app_service)
                }
                .update(post.id, form_data)
                .await
                {
                    | Ok(updated_post) => {
                        let mut posts_write = posts_clone.write();
                        if let Some(index) = posts_write.iter().position(|item| item.id == updated_post.id) {
                            posts_write[index] = updated_post.clone();
                        }
                        selected_post_clone.set(None);
                        form_clone.set(PostForm::default());
                        is_editing_clone.set(false);
                        error_clone.set(None);
                    },
                    | Err(err) => {
                        error_clone.set(Some(format!("Error updating post: {}", err)));
                    },
                }
            });
        }
    };

    let handle_delete = move |id: i32| {
        let mut posts_clone = posts;
        let mut selected_post_clone = selected_post;
        let mut form_clone = form;
        let mut is_editing_clone = is_editing;
        let mut error_clone = error;

        spawn(async move {
            match {
                let repo = JsonPlaceholderPostRepository::new();
                let service = PostService::new(repo);
                let app_service = PostApplicationService::new(service);
                PostApiController::new(app_service)
            }
            .delete(id)
            .await
            {
                | Ok(_) => {
                    posts_clone.write().retain(|post| post.id != id);
                    if selected_post_clone().is_some_and(|p| p.id == id) {
                        selected_post_clone.set(None);
                        form_clone.set(PostForm::default());
                        is_editing_clone.set(false);
                    }
                    error_clone.set(None);
                },
                | Err(err) => {
                    error_clone.set(Some(format!("Error deleting post: {}", err)));
                },
            }
        });
    };

    let mut handle_edit = move |post: Post| {
        selected_post.set(Some(post.clone()));
        form.set(PostForm {
            userId: post.userId,
            title: post.title,
            body: post.body,
        });
        is_editing.set(true);
    };

    let handle_cancel = move |_| {
        form.set(PostForm::default());
        is_editing.set(false);
    };

    rsx! {
        div { class: "resource-page",
            div { class: "resource-header",
                h1 { class: "page-title", "Posts" }
                p { class: "page-kicker", "Create, update, and inspect JSONPlaceholder post records." }
            }

            // Error message
            {error().map(|err| rsx!(
                div { class: "notice",
                    p { {err} }
                }
            ))}

            // Post form
            div { class: "panel form-panel",
                h2 { class: "panel-title",
                    {if is_editing() { "Edit post" } else { "Add new post" }}
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
                    div { class: "field",
                        label { "Body" }
                        textarea {
                            rows: "4",
                            value: form().body.clone(),
                            oninput: move |evt| {
                                let mut form_write = form.write();
                                form_write.body = evt.value().clone();
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
                                "Update Post"
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
                                "Add Post"
                            }
                        }
                    }}
                }
            }

            // Posts list
            div { class: "table-panel",
                div { class: "table-caption",
                    h3 { "Posts" }
                    span { class: "command-tag", "{posts().len()} rows" }
                }
                table {
                    thead {
                        tr {
                            th {  "ID" }
                            th {  "User ID" }
                            th {  "Title" }
                            th {  "Actions" }
                        }
                    }
                    tbody {
                        {posts().into_iter().map(|post| {
                            let post_id = post.id;
                            let post_for_edit = post.clone();
                            rsx!(
                                tr { key: "{post.id}",
                                    td {  {post.id.to_string()} }
                                    td {  {post.userId.to_string()} }
                                    td {  {post.title.clone()} }
                                    td {
                                        div { class: "row-actions",
                                            button {
                                                type: "button",
                                                onclick: move |_| handle_edit(post_for_edit.clone()),
                                                "Edit"
                                            }
                                            button {
                                                type: "button",
                                                onclick: move |_| handle_delete(post_id),
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

            // Post detail view
            {selected_post().map(|post| rsx!(
                div { class: "detail-panel",
                    h3 {  "Post Details" }
                    p { "Title: ", span { {post.title.clone()} } }
                    p { "Body: ", span { {post.body.clone()} } }
                }
            ))}
        }
    }
}
