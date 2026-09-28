use loco_rs::prelude::*;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub use super::_entities::process_indices::{self, ActiveModel, Entity, Model};

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateParams {
    pub name: String,
    pub description: Option<String>,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>,
    pub weight: Option<f64>,
    pub performance_indicator_id: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateParams {
    pub name: Option<String>,
    pub description: Option<String>,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>,
    pub weight: Option<f64>,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub async fn find_by_id(db: &DatabaseConnection, id: i32) -> ModelResult<Self> {
        let item = Entity::find_by_id(id).one(db).await?;
        item.ok_or_else(|| ModelError::EntityNotFound)
    }

    pub async fn find_by_indicator(
        db: &DatabaseConnection,
        indicator_id: i32,
    ) -> ModelResult<Vec<Self>> {
        Ok(Entity::find()
            .filter(process_indices::Column::PerformanceIndicatorId.eq(indicator_id))
            .all(db)
            .await?)
    }

    pub async fn create(db: &DatabaseConnection, params: &CreateParams) -> ModelResult<Self> {
        let item = ActiveModel {
            name: ActiveValue::set(params.name.clone()),
            description: ActiveValue::set(params.description.clone()),
            target_value: ActiveValue::set(params.target_value),
            actual_value: ActiveValue::set(params.actual_value),
            weight: ActiveValue::set(params.weight),
            performance_indicator_id: ActiveValue::set(params.performance_indicator_id),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(item)
    }

    pub async fn update(
        db: &DatabaseConnection,
        id: i32,
        params: &UpdateParams,
    ) -> ModelResult<Self> {
        let item = Self::find_by_id(db, id).await?;
        let mut active: ActiveModel = item.into();

        if let Some(ref name) = params.name {
            active.name = ActiveValue::set(name.clone());
        }
        if params.description.is_some() {
            active.description = ActiveValue::set(params.description.clone());
        }
        if params.target_value.is_some() {
            active.target_value = ActiveValue::set(params.target_value);
        }
        if params.actual_value.is_some() {
            active.actual_value = ActiveValue::set(params.actual_value);
        }
        if params.weight.is_some() {
            active.weight = ActiveValue::set(params.weight);
        }

        Ok(active.update(db).await?)
    }

    pub async fn delete(db: &DatabaseConnection, id: i32) -> ModelResult<()> {
        let item = Self::find_by_id(db, id).await?;
        let active: ActiveModel = item.into();
        active.delete(db).await?;
        Ok(())
    }
}
