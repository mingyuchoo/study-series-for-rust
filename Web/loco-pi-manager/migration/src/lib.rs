#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;
mod m20220101_000001_users;
mod m20240101_000002_performance_indicators;
mod m20240101_000003_input_indices;
mod m20240101_000004_process_indices;
mod m20240101_000005_output_indices;
mod m20240101_000006_outcome_indices;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_users::Migration),
            Box::new(m20240101_000002_performance_indicators::Migration),
            Box::new(m20240101_000003_input_indices::Migration),
            Box::new(m20240101_000004_process_indices::Migration),
            Box::new(m20240101_000005_output_indices::Migration),
            Box::new(m20240101_000006_outcome_indices::Migration),
            // inject-above (do not remove this comment)
        ]
    }
}
