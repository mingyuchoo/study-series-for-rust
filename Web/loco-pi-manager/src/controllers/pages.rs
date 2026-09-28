use axum::extract::Path;
use loco_rs::prelude::*;

use crate::models::performance_indicators;

/// GET / - 대시보드 (성과지표 목록)
async fn dashboard(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let items = performance_indicators::Model::find_all(&ctx.db)
        .await
        .unwrap_or_default();
    format::render().view(
        &v,
        "dashboard/index.html",
        serde_json::json!({ "indicators": items }),
    )
}

/// GET /indicators/:id - 성과지표 상세
async fn indicator_detail(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
) -> Result<Response> {
    let item = performance_indicators::Model::find_by_id(&ctx.db, id).await?;
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

    format::render().view(
        &v,
        "dashboard/detail.html",
        serde_json::json!({
            "indicator": item,
            "score": score,
            "input_indices": input,
            "process_indices": process,
            "output_indices": output,
            "outcome_indices": outcome,
        }),
    )
}

/// GET /indicators/new - 성과지표 생성 폼
async fn indicator_new(ViewEngine(v): ViewEngine<TeraView>) -> Result<Response> {
    format::render().view(&v, "dashboard/new.html", serde_json::json!({}))
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/", get(dashboard))
        .add("/indicators/new", get(indicator_new))
        .add("/indicators/{id}", get(indicator_detail))
}
