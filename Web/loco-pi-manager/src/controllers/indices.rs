use axum::extract::Path;
use loco_rs::prelude::*;

use crate::models::{input_indices, outcome_indices, output_indices, process_indices};

// ── Input Indices ──────────────────────────────────────────

async fn list_input(State(ctx): State<AppContext>, Path(pi_id): Path<i32>) -> Result<Response> {
    let items = input_indices::Model::find_by_indicator(&ctx.db, pi_id).await?;
    format::json(items)
}

async fn create_input(
    State(ctx): State<AppContext>,
    Json(params): Json<input_indices::CreateParams>,
) -> Result<Response> {
    let item = input_indices::Model::create(&ctx.db, &params).await?;
    format::json(item)
}

async fn update_input(
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
    Json(params): Json<input_indices::UpdateParams>,
) -> Result<Response> {
    let item = input_indices::Model::update(&ctx.db, id, &params).await?;
    format::json(item)
}

async fn remove_input(State(ctx): State<AppContext>, Path(id): Path<i32>) -> Result<Response> {
    input_indices::Model::delete(&ctx.db, id).await?;
    format::empty_json()
}

// ── Process Indices ────────────────────────────────────────

async fn list_process(State(ctx): State<AppContext>, Path(pi_id): Path<i32>) -> Result<Response> {
    let items = process_indices::Model::find_by_indicator(&ctx.db, pi_id).await?;
    format::json(items)
}

async fn create_process(
    State(ctx): State<AppContext>,
    Json(params): Json<process_indices::CreateParams>,
) -> Result<Response> {
    let item = process_indices::Model::create(&ctx.db, &params).await?;
    format::json(item)
}

async fn update_process(
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
    Json(params): Json<process_indices::UpdateParams>,
) -> Result<Response> {
    let item = process_indices::Model::update(&ctx.db, id, &params).await?;
    format::json(item)
}

async fn remove_process(State(ctx): State<AppContext>, Path(id): Path<i32>) -> Result<Response> {
    process_indices::Model::delete(&ctx.db, id).await?;
    format::empty_json()
}

// ── Output Indices ─────────────────────────────────────────

async fn list_output(State(ctx): State<AppContext>, Path(pi_id): Path<i32>) -> Result<Response> {
    let items = output_indices::Model::find_by_indicator(&ctx.db, pi_id).await?;
    format::json(items)
}

async fn create_output(
    State(ctx): State<AppContext>,
    Json(params): Json<output_indices::CreateParams>,
) -> Result<Response> {
    let item = output_indices::Model::create(&ctx.db, &params).await?;
    format::json(item)
}

async fn update_output(
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
    Json(params): Json<output_indices::UpdateParams>,
) -> Result<Response> {
    let item = output_indices::Model::update(&ctx.db, id, &params).await?;
    format::json(item)
}

async fn remove_output(State(ctx): State<AppContext>, Path(id): Path<i32>) -> Result<Response> {
    output_indices::Model::delete(&ctx.db, id).await?;
    format::empty_json()
}

// ── Outcome Indices ────────────────────────────────────────

async fn list_outcome(State(ctx): State<AppContext>, Path(pi_id): Path<i32>) -> Result<Response> {
    let items = outcome_indices::Model::find_by_indicator(&ctx.db, pi_id).await?;
    format::json(items)
}

async fn create_outcome(
    State(ctx): State<AppContext>,
    Json(params): Json<outcome_indices::CreateParams>,
) -> Result<Response> {
    let item = outcome_indices::Model::create(&ctx.db, &params).await?;
    format::json(item)
}

async fn update_outcome(
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
    Json(params): Json<outcome_indices::UpdateParams>,
) -> Result<Response> {
    let item = outcome_indices::Model::update(&ctx.db, id, &params).await?;
    format::json(item)
}

async fn remove_outcome(State(ctx): State<AppContext>, Path(id): Path<i32>) -> Result<Response> {
    outcome_indices::Model::delete(&ctx.db, id).await?;
    format::empty_json()
}

// ── Routes ─────────────────────────────────────────────────

pub fn input_routes() -> Routes {
    Routes::new()
        .prefix("/api/input-indices")
        .add("/by-indicator/{pi_id}", get(list_input))
        .add("/", post(create_input))
        .add("/{id}", put(update_input))
        .add("/{id}", delete(remove_input))
}

pub fn process_routes() -> Routes {
    Routes::new()
        .prefix("/api/process-indices")
        .add("/by-indicator/{pi_id}", get(list_process))
        .add("/", post(create_process))
        .add("/{id}", put(update_process))
        .add("/{id}", delete(remove_process))
}

pub fn output_routes() -> Routes {
    Routes::new()
        .prefix("/api/output-indices")
        .add("/by-indicator/{pi_id}", get(list_output))
        .add("/", post(create_output))
        .add("/{id}", put(update_output))
        .add("/{id}", delete(remove_output))
}

pub fn outcome_routes() -> Routes {
    Routes::new()
        .prefix("/api/outcome-indices")
        .add("/by-indicator/{pi_id}", get(list_outcome))
        .add("/", post(create_outcome))
        .add("/{id}", put(update_outcome))
        .add("/{id}", delete(remove_outcome))
}
