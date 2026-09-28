use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        create_table(
            m,
            "output_indices",
            &[
                ("id", ColType::PkAuto),
                ("name", ColType::String),
                ("description", ColType::TextNull),
                ("target_value", ColType::FloatNull),
                ("actual_value", ColType::FloatNull),
                ("weight", ColType::FloatNull),
            ],
            &[("performance_indicator", "")],
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        drop_table(m, "output_indices").await?;
        Ok(())
    }
}
