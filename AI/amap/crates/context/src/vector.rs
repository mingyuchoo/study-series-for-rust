//! Semantic retrieval with an OpenAI-compatible embedding provider and an offline fallback.
use crate::Hit;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const FALLBACK_DIM: usize = 512;

pub fn fallback_embed(text: &str) -> Vec<f32> {
    let mut vector = vec![0f32; FALLBACK_DIM];
    for token in text
        .to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| token.len() > 1)
    {
        let mut hash: u64 = 1469598103934665603;
        for byte in token.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(1099511628211);
        }
        vector[(hash % FALLBACK_DIM as u64) as usize] += 1.0;
    }
    normalize(&mut vector);
    vector
}

fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        vector.iter_mut().for_each(|value| *value /= norm);
    }
}

pub trait VectorIndex: Send + Sync {
    fn add(&mut self, id: &str, kind: &str, text: &str);
    fn add_embedding(&mut self, id: &str, kind: &str, embedding: Vec<f32>);
    fn search(&self, query: &str, limit: usize) -> Vec<Hit>;
    fn search_embedding(&self, embedding: &[f32], limit: usize) -> Vec<Hit>;
}

#[derive(Default)]
pub struct InMemoryVectorIndex {
    rows: HashMap<String, (String, Vec<f32>)>,
}

impl VectorIndex for InMemoryVectorIndex {
    fn add(&mut self, id: &str, kind: &str, text: &str) {
        self.add_embedding(id, kind, fallback_embed(text));
    }

    fn add_embedding(&mut self, id: &str, kind: &str, mut embedding: Vec<f32>) {
        normalize(&mut embedding);
        self.rows
            .insert(id.to_string(), (kind.to_string(), embedding));
    }

    fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        self.search_embedding(&fallback_embed(query), limit)
    }

    fn search_embedding(&self, query: &[f32], limit: usize) -> Vec<Hit> {
        let mut hits: Vec<Hit> = self
            .rows
            .iter()
            .filter(|(_, (_, vector))| vector.len() == query.len())
            .map(|(id, (kind, vector))| Hit {
                id: id.clone(),
                kind: kind.clone(),
                score: vector.iter().zip(query).map(|(a, b)| a * b).sum(),
            })
            .filter(|hit| hit.score > 0.0)
            .collect();
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(limit);
        hits
    }
}

/// How the API key is sent to the embedding endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbeddingAuth {
    /// `Authorization: Bearer <key>` (OpenAI and most compatible servers).
    Bearer,
    /// `api-key: <key>` (Azure OpenAI).
    ApiKeyHeader,
}

#[derive(Clone)]
pub struct OpenAiEmbeddingProvider {
    client: reqwest::Client,
    endpoint: String,
    api_key: Option<String>,
    auth: EmbeddingAuth,
    model: String,
}

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    index: usize,
    embedding: Vec<f32>,
}

impl OpenAiEmbeddingProvider {
    /// Generic OpenAI-compatible provider from `AMAP_EMBEDDING_URL` (+ optional
    /// `AMAP_EMBEDDING_API_KEY`, `AMAP_EMBEDDING_MODEL`). When that is unset, falls back to
    /// Azure OpenAI via [`Self::azure_from_env`].
    pub fn from_env() -> Option<Self> {
        let Ok(endpoint) = std::env::var("AMAP_EMBEDDING_URL") else {
            return Self::azure_from_env();
        };
        Some(Self {
            client: reqwest::Client::new(),
            endpoint,
            api_key: std::env::var("AMAP_EMBEDDING_API_KEY").ok(),
            auth: EmbeddingAuth::Bearer,
            model: std::env::var("AMAP_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "text-embedding-3-small".into()),
        })
    }

    /// Azure OpenAI embeddings from `AZURE_OPENAI_API_KEY`, `AZURE_OPENAI_ENDPOINT` and
    /// `AZURE_OPENAI_EMBEDDING_DEPLOYMENT` (all required) plus optional `AZURE_OPENAI_API_VERSION`.
    pub fn azure_from_env() -> Option<Self> {
        let api_key = std::env::var("AZURE_OPENAI_API_KEY").ok()?;
        let endpoint = std::env::var("AZURE_OPENAI_ENDPOINT").ok()?;
        let deployment = std::env::var("AZURE_OPENAI_EMBEDDING_DEPLOYMENT").ok()?;
        let api_version = std::env::var("AZURE_OPENAI_API_VERSION")
            .unwrap_or_else(|_| "2024-12-01-preview".into());
        Some(Self::azure(api_key, endpoint, deployment, api_version))
    }

    /// Azure OpenAI embeddings against `{endpoint}/openai/deployments/{deployment}/embeddings`.
    pub fn azure(
        api_key: impl Into<String>,
        endpoint: impl Into<String>,
        deployment: impl Into<String>,
        api_version: impl Into<String>,
    ) -> Self {
        let deployment = deployment.into();
        Self {
            client: reqwest::Client::new(),
            endpoint: format!(
                "{}/openai/deployments/{}/embeddings?api-version={}",
                endpoint.into().trim_end_matches('/'),
                deployment,
                api_version.into()
            ),
            api_key: Some(api_key.into()),
            auth: EmbeddingAuth::ApiKeyHeader,
            model: deployment,
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub async fn embed(&self, input: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let mut request = self.client.post(&self.endpoint).json(&EmbeddingRequest {
            model: &self.model,
            input,
        });
        if let Some(api_key) = &self.api_key {
            request = match self.auth {
                EmbeddingAuth::Bearer => request.bearer_auth(api_key),
                EmbeddingAuth::ApiKeyHeader => request.header("api-key", api_key),
            };
        }
        let response = request
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json::<EmbeddingResponse>()
            .await
            .map_err(|error| error.to_string())?;
        let mut rows = response.data;
        rows.sort_by_key(|row| row.index);
        if rows.len() != input.len() || rows.iter().any(|row| row.embedding.is_empty()) {
            return Err("embedding provider returned an invalid batch".into());
        }
        Ok(rows.into_iter().map(|row| row.embedding).collect())
    }
}

#[cfg(test)]
mod azure_tests {
    use super::*;

    #[test]
    fn azure_endpoint_is_deployment_scoped_with_api_version() {
        let p = OpenAiEmbeddingProvider::azure(
            "k",
            "https://example.cognitiveservices.azure.com/",
            "text-embedding-3-large",
            "2024-12-01-preview",
        );
        assert_eq!(
            p.endpoint(),
            "https://example.cognitiveservices.azure.com/openai/deployments/text-embedding-3-large/embeddings?api-version=2024-12-01-preview"
        );
        assert_eq!(p.auth, EmbeddingAuth::ApiKeyHeader);
        assert_eq!(p.model, "text-embedding-3-large");
    }
}
