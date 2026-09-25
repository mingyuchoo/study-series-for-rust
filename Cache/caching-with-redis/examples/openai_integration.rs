// OpenAI API 통합 + 캐시 성능 예제
use anyhow::Result;
use caching_with_redis::EmbeddingCache;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use std::time::Instant;

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    input: String,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

struct OpenAIClient {
    client: Client,
    api_key: String,
}

impl OpenAIClient {
    fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let request = OpenAIRequest {
            model: "text-embedding-ada-002".to_string(),
            input: text.to_string(),
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<OpenAIResponse>()
            .await?;

        Ok(response.data[0].embedding.clone())
    }
}

fn preview(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    t
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY 환경변수를 설정해주세요");
    let openai_client = Arc::new(OpenAIClient::new(api_key));
    let cache = EmbeddingCache::new("redis://127.0.0.1:6379", 3600).await?;

    // 테스트 텍스트들
    let texts = [
        "Rust는 시스템 프로그래밍 언어입니다",
        "Redis는 인메모리 데이터 저장소입니다",
        "임베딩은 텍스트를 벡터로 변환합니다",
        "캐싱은 성능을 크게 향상시킵니다",
        "AI 애플리케이션에서 속도는 중요합니다",
    ];

    println!("🚀 OpenAI 임베딩 캐시 성능 테스트\n=====================================");

    // 첫 번째 실행 (새로 계산)
    println!("\n📊 첫 번째 실행 (API 호출):");
    let start = Instant::now();
    for (i, text) in texts.iter().enumerate() {
        let client = openai_client.clone();
        let embedding_func = move |t: String| {
            let client = client.clone();
            async move { client.get_embedding(&t).await }
        };
        let embedding = cache
            .get_or_compute(text, "openai-ada-002", embedding_func)
            .await?;
        println!(
            "  {}. {} -> {}차원",
            i + 1,
            preview(text, 30),
            embedding.len()
        );
    }
    let first_duration = start.elapsed();
    println!("  총 시간: {:?}", first_duration);

    // 두 번째 실행 (캐시에서 조회)
    println!("\n⚡ 두 번째 실행 (캐시 조회):");
    let start = Instant::now();
    for (i, text) in texts.iter().enumerate() {
        let client = openai_client.clone();
        let embedding_func = move |t: String| {
            let client = client.clone();
            async move { client.get_embedding(&t).await }
        };
        let embedding = cache
            .get_or_compute(text, "openai-ada-002", embedding_func)
            .await?;
        println!(
            "  {}. {} -> {}차원",
            i + 1,
            preview(text, 30),
            embedding.len()
        );
    }
    let second_duration = start.elapsed();
    println!("  총 시간: {:?}", second_duration);

    // 성능 비교
    let speedup = first_duration.as_millis() as f64 / second_duration.as_millis() as f64;
    println!("\n📈 성능 결과:");
    println!("  첫 번째 (API): {:?}", first_duration);
    println!("  두 번째 (캐시): {:?}", second_duration);
    println!("  속도 향상: {:.1}x", speedup);
    println!("  시간 절약: {:.1}%", (1.0 - 1.0 / speedup) * 100.0);

    // 캐시 통계
    let stats = cache.get_stats().await;
    println!("\n📊 캐시 통계:");
    println!("  히트: {}", stats.hit_count);
    println!("  미스: {}", stats.miss_count);
    println!("  히트율: {:.1}%", stats.hit_rate() * 100.0);

    Ok(())
}
