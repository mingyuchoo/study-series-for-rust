// CLI 및 데모
use anyhow::Result;
use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use std::time::{SystemTime, UNIX_EPOCH};

use caching_with_redis::config::CacheConfig;
use caching_with_redis::utils::*;
use caching_with_redis::{
    AzureOpenAI, ChatMessage, CompressedEmbeddingCache, EmbeddingCache, ResponseCache, SessionStore,
};

#[derive(Parser)]
#[command(name = "caching_with_redis")]
#[command(about = "Redis + Azure OpenAI 캐싱 데모 CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Redis 서버 설정 및 시작 (Docker 필요)
    Setup {
        #[arg(short, long, default_value = "512mb")]
        memory: String,
    },
    /// Redis 연결 상태 테스트
    Status,
    /// 캐시 통계 출력 (emb:*)
    Stats,
    /// 캐시 내용 삭제 (emb:*, cemb:*, resp:*, sess:*)
    Clear,
    /// 임베딩 캐시 테스트 (Azure OpenAI 사용 가능 시 실제 호출)
    Test {
        #[arg(short, long, default_value = "안녕하세요, Redis 임베딩 캐시입니다!")]
        text: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    env_logger::init();

    let cli = Cli::parse();
    let cfg = CacheConfig::from_env();

    match cli.command {
        Commands::Setup { memory } => {
            setup_redis_docker(&memory)?;
        }
        Commands::Status => {
            if let Err(_e) = test_redis_connection(&cfg.redis_url).await {
                eprintln!(
                    "Redis가 실행되어 있지 않습니다. Redis를 먼저 실행하세요. (URL: {})",
                    cfg.redis_url
                );
            }
        }
        Commands::Stats => {
            let cache = EmbeddingCache::new(&cfg.redis_url, cfg.ttl_seconds).await?;
            let s = cache.get_stats().await;
            println!(
                "임베딩 캐시 통계: total_keys={}, hit={}, miss={}, hit_rate={:.2}%",
                s.total_keys,
                s.hit_count,
                s.miss_count,
                s.hit_rate() * 100.0
            );
            let cc = CompressedEmbeddingCache::new(&cfg.redis_url, cfg.ttl_seconds, 6).await?;
            if let Ok(cs) = cc.get_compression_stats().await {
                println!(
                    "압축 캐시 샘플: count={}, avg={}B, est_uncompressed={}B, ratio={:.2}x, saved={:.1}%",
                    cs.sample_count,
                    cs.avg_compressed_size,
                    cs.estimated_uncompressed_size,
                    cs.compression_ratio,
                    cs.memory_saved_percent()
                );
            }
        }
        Commands::Clear => {
            let ec = EmbeddingCache::new(&cfg.redis_url, cfg.ttl_seconds).await?;
            ec.clear().await?;
            let rc = ResponseCache::new(&cfg.redis_url, cfg.ttl_seconds).await?;
            rc.clear().await?;
            let sc = SessionStore::new(&cfg.redis_url, cfg.ttl_seconds).await?;
            sc.clear_all().await?;
            let cc = CompressedEmbeddingCache::new(&cfg.redis_url, cfg.ttl_seconds, 6).await?;
            cc.clear_all().await?;
            println!("모든 캐시 키 삭제 완료");
        }
        Commands::Test { text } => {
            // 1) 임베딩 캐시 사용 + Azure OpenAI 호출 (가능하면)
            let cache = EmbeddingCache::new(&cfg.redis_url, cfg.ttl_seconds).await?;
            let model = "azure-embedding";
            let maybe_client = match (
                &cfg.azure_openai_endpoint,
                &cfg.azure_openai_api_key,
                &cfg.azure_openai_embeddings_deployment,
                &cfg.azure_openai_chat_deployment,
            ) {
                (Some(e), Some(k), Some(ed), Some(cd)) => Some(AzureOpenAI::new(
                    e.clone(),
                    k.clone(),
                    ed.clone(),
                    cd.clone(),
                )),
                _ => None,
            };

            let compute = |t: String| async {
                if let Some(client) = &maybe_client {
                    client.embed(t).await
                } else {
                    Ok(vec![0.1, 0.2, 0.3, 0.4, 0.5])
                }
            };
            let emb = cache.get_or_compute(&text, model, compute).await?;
            println!("임베딩 길이: {}", emb.len());

            // 2) 질의-응답 캐시 + 세션 저장
            let resp_cache = ResponseCache::new(&cfg.redis_url, cfg.ttl_seconds).await?;
            let answer = if let Some(a) = resp_cache.get(&text).await? {
                a
            } else {
                let a = if let Some(client) = &maybe_client {
                    client
                        .chat("당신은 유능한 도우미입니다.", &text)
                        .await
                        .unwrap_or_else(|_| "(더미 응답) 안녕하세요!".to_string())
                } else {
                    "(더미 응답) 안녕하세요!".to_string()
                };
                resp_cache.set(&text, &a).await?;
                a
            };
            println!("응답: {}", answer);

            let store = SessionStore::new(&cfg.redis_url, cfg.ttl_seconds).await?;
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            store
                .append_message(
                    "user-1",
                    &ChatMessage {
                        role: "user".into(),
                        content: text.clone(),
                        ts: now_ms,
                    },
                )
                .await?;
            store
                .append_message(
                    "user-1",
                    &ChatMessage {
                        role: "assistant".into(),
                        content: answer.clone(),
                        ts: now_ms,
                    },
                )
                .await?;
            let hist = store.get_history("user-1", 10).await?;
            println!("최근 대화 {}개", hist.len());
        }
    }

    Ok(())
}
