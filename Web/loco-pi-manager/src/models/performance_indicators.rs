use loco_rs::prelude::*;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub use super::_entities::performance_indicators::{self, ActiveModel, Entity, Model};

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateParams {
    pub name: String,
    pub description: Option<String>,
    pub year: i32,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>,
    pub unit: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateParams {
    pub name: Option<String>,
    pub description: Option<String>,
    pub year: Option<i32>,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>,
    pub unit: Option<String>,
    pub status: Option<String>,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub async fn find_by_id(db: &DatabaseConnection, id: i32) -> ModelResult<Self> {
        let item = Entity::find_by_id(id).one(db).await?;
        item.ok_or_else(|| ModelError::EntityNotFound)
    }

    pub async fn find_all(db: &DatabaseConnection) -> ModelResult<Vec<Self>> {
        Ok(Entity::find().all(db).await?)
    }

    pub async fn create(db: &DatabaseConnection, params: &CreateParams) -> ModelResult<Self> {
        let item = ActiveModel {
            name: ActiveValue::set(params.name.clone()),
            description: ActiveValue::set(params.description.clone()),
            year: ActiveValue::set(params.year),
            target_value: ActiveValue::set(params.target_value),
            actual_value: ActiveValue::set(params.actual_value),
            unit: ActiveValue::set(params.unit.clone()),
            status: ActiveValue::set(params.status.clone()),
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
        if let Some(year) = params.year {
            active.year = ActiveValue::set(year);
        }
        if params.target_value.is_some() {
            active.target_value = ActiveValue::set(params.target_value);
        }
        if params.actual_value.is_some() {
            active.actual_value = ActiveValue::set(params.actual_value);
        }
        if params.unit.is_some() {
            active.unit = ActiveValue::set(params.unit.clone());
        }
        if params.status.is_some() {
            active.status = ActiveValue::set(params.status.clone());
        }

        Ok(active.update(db).await?)
    }

    pub async fn delete(db: &DatabaseConnection, id: i32) -> ModelResult<()> {
        let item = Self::find_by_id(db, id).await?;
        let active: ActiveModel = item.into();
        active.delete(db).await?;
        Ok(())
    }

    /// 각 하위지표(입력/과정/산출/결과)의 가중평균을 계산하여 최종 성과점수를 산출
    pub async fn calculate_score(&self, db: &DatabaseConnection) -> ModelResult<f64> {
        use super::_entities::{input_indices, outcome_indices, output_indices, process_indices};

        let input = input_indices::Entity::find()
            .filter(input_indices::Column::PerformanceIndicatorId.eq(self.id))
            .all(db)
            .await?;
        let process = process_indices::Entity::find()
            .filter(process_indices::Column::PerformanceIndicatorId.eq(self.id))
            .all(db)
            .await?;
        let output = output_indices::Entity::find()
            .filter(output_indices::Column::PerformanceIndicatorId.eq(self.id))
            .all(db)
            .await?;
        let outcome = outcome_indices::Entity::find()
            .filter(outcome_indices::Column::PerformanceIndicatorId.eq(self.id))
            .all(db)
            .await?;

        let input_score = weighted_average(&input);
        let process_score = weighted_average(&process);
        let output_score = weighted_average(&output);
        let outcome_score = weighted_average(&outcome);

        let scores = [input_score, process_score, output_score, outcome_score];
        let valid: Vec<f64> = scores.iter().filter_map(|s| *s).collect();

        if valid.is_empty() {
            return Ok(0.0);
        }

        Ok(valid.iter().sum::<f64>() / valid.len() as f64)
    }
}

/// 가중평균 계산 헬퍼: actual_value * weight 의 합 / weight 합
fn weighted_average<T: HasWeightedValue>(items: &[T]) -> Option<f64> {
    if items.is_empty() {
        return None;
    }

    let mut total_weight = 0.0;
    let mut weighted_sum = 0.0;

    for item in items {
        let w = item.weight_val().unwrap_or(1.0);
        let actual = item.actual_val().unwrap_or(0.0);
        let target = item.target_val().unwrap_or(1.0);

        if target > 0.0 {
            weighted_sum += (actual / target) * 100.0 * w;
            total_weight += w;
        }
    }

    if total_weight > 0.0 {
        Some(weighted_sum / total_weight)
    } else {
        None
    }
}

trait HasWeightedValue {
    fn weight_val(&self) -> Option<f64>;
    fn actual_val(&self) -> Option<f64>;
    fn target_val(&self) -> Option<f64>;
}

macro_rules! impl_weighted_value {
    ($entity_mod:path) => {
        impl HasWeightedValue for $entity_mod {
            fn weight_val(&self) -> Option<f64> {
                self.weight
            }
            fn actual_val(&self) -> Option<f64> {
                self.actual_value
            }
            fn target_val(&self) -> Option<f64> {
                self.target_value
            }
        }
    };
}

impl_weighted_value!(super::_entities::input_indices::Model);
impl_weighted_value!(super::_entities::process_indices::Model);
impl_weighted_value!(super::_entities::output_indices::Model);
impl_weighted_value!(super::_entities::outcome_indices::Model);
