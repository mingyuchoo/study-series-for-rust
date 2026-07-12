use sqlx::{PgPool,
           postgres::PgPoolOptions};
use std::env;
use tracing::info;

pub async fn init_db() -> anyhow::Result<PgPool> {
    let database_url = env::var("DATABASE_URL")?;

    let pool = PgPoolOptions::new().max_connections(5).connect(&database_url).await?;

    // Run migrations automatically on startup
    sqlx::migrate!().run(&pool).await?;
    info!("Connected to database and migrations applied!");
    Ok(pool)
}
