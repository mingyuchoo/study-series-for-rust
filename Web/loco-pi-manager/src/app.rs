use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Hooks, Initializer},
    bgworker::{BackgroundWorker, Queue},
    boot::{create_app, BootResult, StartMode},
    config::Config,
    controller::AppRoutes,
    db::{self, truncate_table},
    environment::Environment,
    task::Tasks,
    Result,
};
use migration::Migrator;
use std::path::Path;

#[allow(unused_imports)]
use crate::{
    controllers, initializers,
    models::_entities::{
        input_indices, outcome_indices, output_indices, performance_indicators, process_indices,
        users,
    },
    tasks,
    workers::downloader::DownloadWorker,
};

pub struct App;
#[async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        env!("CARGO_CRATE_NAME")
    }

    fn app_version() -> String {
        format!(
            "{} ({})",
            env!("CARGO_PKG_VERSION"),
            option_env!("BUILD_SHA")
                .or(option_env!("GITHUB_SHA"))
                .unwrap_or("dev")
        )
    }

    async fn boot(
        mode: StartMode,
        environment: &Environment,
        config: Config,
    ) -> Result<BootResult> {
        create_app::<Self, Migrator>(mode, environment, config).await
    }

    async fn initializers(_ctx: &AppContext) -> Result<Vec<Box<dyn Initializer>>> {
        Ok(vec![
            Box::new(initializers::view_engine::ViewEngineInitializer),
            Box::new(initializers::seed_data::SeedDataInitializer),
        ])
    }

    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::with_default_routes() // controller routes below
            .add_route(controllers::auth::routes())
            .add_route(controllers::performance_indicators::routes())
            .add_route(controllers::indices::input_routes())
            .add_route(controllers::indices::process_routes())
            .add_route(controllers::indices::output_routes())
            .add_route(controllers::indices::outcome_routes())
            .add_route(controllers::ai_assistant::routes())
            .add_route(controllers::pages::routes())
    }
    async fn connect_workers(ctx: &AppContext, queue: &Queue) -> Result<()> {
        queue.register(DownloadWorker::build(ctx)).await?;
        Ok(())
    }

    #[allow(unused_variables)]
    fn register_tasks(tasks: &mut Tasks) {
        // tasks-inject (do not remove)
    }
    async fn truncate(ctx: &AppContext) -> Result<()> {
        // 외래 키 제약 때문에 자식 테이블부터 삭제
        truncate_table(&ctx.db, outcome_indices::Entity).await?;
        truncate_table(&ctx.db, output_indices::Entity).await?;
        truncate_table(&ctx.db, process_indices::Entity).await?;
        truncate_table(&ctx.db, input_indices::Entity).await?;
        truncate_table(&ctx.db, performance_indicators::Entity).await?;
        truncate_table(&ctx.db, users::Entity).await?;
        Ok(())
    }
    async fn seed(ctx: &AppContext, base: &Path) -> Result<()> {
        db::seed::<users::ActiveModel>(&ctx.db, &base.join("users.yaml").display().to_string())
            .await?;

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

        Ok(())
    }
}
