use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    db,
    environment::Environment,
    Result,
};
use sea_orm::{EntityTrait, PaginatorTrait};
use std::path::Path;
use tracing::info;

use crate::models::_entities::{
    input_indices, outcome_indices, output_indices, performance_indicators, process_indices,
};

pub struct SeedDataInitializer;

#[async_trait]
impl Initializer for SeedDataInitializer {
    fn name(&self) -> String {
        "seed-data".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        // 테스트 환경에서는 시드를 건너뜁니다
        if ctx.environment == Environment::Test {
            info!("테스트 환경이므로 자동 시드를 건너뜁니다.");
            return Ok(());
        }

        // 기존 데이터가 없을 때만 시드 데이터를 로드합니다
        let count = performance_indicators::Entity::find()
            .count(&ctx.db)
            .await?;

        if count == 0 {
            info!("시드 데이터를 로드합니다...");

            let base = Path::new("src/fixtures");

            // 순서대로 시드 (외래 키 제약 때문에 부모 테이블부터)
            db::seed::<performance_indicators::ActiveModel>(
                &ctx.db,
                &base
                    .join("performance_indicators.yaml")
                    .display()
                    .to_string(),
            )
            .await?;

            db::seed::<input_indices::ActiveModel>(
                &ctx.db,
                &base.join("input_indices.yaml").display().to_string(),
            )
            .await?;

            db::seed::<process_indices::ActiveModel>(
                &ctx.db,
                &base.join("process_indices.yaml").display().to_string(),
            )
            .await?;

            db::seed::<output_indices::ActiveModel>(
                &ctx.db,
                &base.join("output_indices.yaml").display().to_string(),
            )
            .await?;

            db::seed::<outcome_indices::ActiveModel>(
                &ctx.db,
                &base.join("outcome_indices.yaml").display().to_string(),
            )
            .await?;

            info!("시드 데이터 로드 완료!");
        } else {
            info!(
                "기존 데이터가 있습니다. 시드를 건너뜁니다. (count: {})",
                count
            );
        }

        Ok(())
    }
}
