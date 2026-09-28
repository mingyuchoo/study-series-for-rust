use serde::{Deserialize, Serialize};

use crate::models::_entities::{
    input_indices, outcome_indices, output_indices, performance_indicators, process_indices,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct IndicatorResponse {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub year: i32,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>,
    pub unit: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndicatorDetailResponse {
    pub indicator: IndicatorResponse,
    pub score: f64,
    pub input_indices: Vec<IndexResponse>,
    pub process_indices: Vec<IndexResponse>,
    pub output_indices: Vec<IndexResponse>,
    pub outcome_indices: Vec<IndexResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexResponse {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>,
    pub weight: Option<f64>,
    pub performance_indicator_id: i32,
}

pub fn item_response(item: performance_indicators::Model) -> IndicatorResponse {
    IndicatorResponse {
        id: item.id,
        name: item.name,
        description: item.description,
        year: item.year,
        target_value: item.target_value,
        actual_value: item.actual_value,
        unit: item.unit,
        status: item.status,
    }
}

pub fn list_response(items: Vec<performance_indicators::Model>) -> Vec<IndicatorResponse> {
    items.into_iter().map(item_response).collect()
}

fn to_index_response<F>(items: Vec<F>) -> Vec<IndexResponse>
where
    F: Into<IndexResponse>,
{
    items.into_iter().map(|i| i.into()).collect()
}

pub fn detail_response(
    item: performance_indicators::Model,
    score: f64,
    input: Vec<input_indices::Model>,
    process: Vec<process_indices::Model>,
    output: Vec<output_indices::Model>,
    outcome: Vec<outcome_indices::Model>,
) -> IndicatorDetailResponse {
    IndicatorDetailResponse {
        indicator: item_response(item),
        score,
        input_indices: to_index_response(input),
        process_indices: to_index_response(process),
        output_indices: to_index_response(output),
        outcome_indices: to_index_response(outcome),
    }
}

macro_rules! impl_into_index_response {
    ($model:path) => {
        impl From<$model> for IndexResponse {
            fn from(m: $model) -> Self {
                IndexResponse {
                    id: m.id,
                    name: m.name,
                    description: m.description,
                    target_value: m.target_value,
                    actual_value: m.actual_value,
                    weight: m.weight,
                    performance_indicator_id: m.performance_indicator_id,
                }
            }
        }
    };
}

impl_into_index_response!(input_indices::Model);
impl_into_index_response!(process_indices::Model);
impl_into_index_response!(output_indices::Model);
impl_into_index_response!(outcome_indices::Model);
