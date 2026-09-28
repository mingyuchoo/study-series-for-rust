use axum::extract::Path;
use loco_rs::prelude::*;

use crate::models::performance_indicators::{CreateParams, Model, UpdateParams};
use crate::views::performance_indicators as views;

/// GET /api/performance-indicators
async fn list(State(ctx): State<AppContext>) -> Result<Response> {
    let items = Model::find_all(&ctx.db).await?;
    format::json(views::list_response(items))
}

/// GET /api/performance-indicators/:id
async fn get_one(State(ctx): State<AppContext>, Path(id): Path<i32>) -> Result<Response> {
    let item = Model::find_by_id(&ctx.db, id).await?;
    let score = item.calculate_score(&ctx.db).await.unwrap_or(0.0);

    let input = crate::models::input_indices::Model::find_by_indicator(&ctx.db, id)
        .await
        .unwrap_or_default();
    let process = crate::models::process_indices::Model::find_by_indicator(&ctx.db, id)
        .await
        .unwrap_or_default();
    let output = crate::models::output_indices::Model::find_by_indicator(&ctx.db, id)
        .await
        .unwrap_or_default();
    let outcome = crate::models::outcome_indices::Model::find_by_indicator(&ctx.db, id)
        .await
        .unwrap_or_default();

    format::json(views::detail_response(
        item, score, input, process, output, outcome,
    ))
}

/// POST /api/performance-indicators
async fn create(
    State(ctx): State<AppContext>,
    Json(params): Json<CreateParams>,
) -> Result<Response> {
    let item = Model::create(&ctx.db, &params).await?;
    format::json(views::item_response(item))
}

/// PUT /api/performance-indicators/:id
async fn update(
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
    Json(params): Json<UpdateParams>,
) -> Result<Response> {
    let item = Model::update(&ctx.db, id, &params).await?;
    format::json(views::item_response(item))
}

/// DELETE /api/performance-indicators/:id
async fn remove(State(ctx): State<AppContext>, Path(id): Path<i32>) -> Result<Response> {
    Model::delete(&ctx.db, id).await?;
    format::empty_json()
}

/// GET /api/performance-indicators/:id/score
async fn score(State(ctx): State<AppContext>, Path(id): Path<i32>) -> Result<Response> {
    let item = Model::find_by_id(&ctx.db, id).await?;
    let score = item.calculate_score(&ctx.db).await.unwrap_or(0.0);
    format::json(serde_json::json!({ "id": id, "score": score }))
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/api/performance-indicators")
        .add("/", get(list))
        .add("/", post(create))
        .add("/{id}", get(get_one))
        .add("/{id}", put(update))
        .add("/{id}", delete(remove))
        .add("/{id}/score", get(score))
}
