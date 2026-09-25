// 성능 비교 벤치마크 예제
use anyhow::Result;
use caching_with_redis::{CompressedEmbeddingCache, EmbeddingCache};
use std::time::Instant;
use tokio::task::JoinSet;

async fn benchmark_normal_cache() -> Result<()> {
    println!("🏃 일반 캐시 벤치마크...");

    let cache = EmbeddingCache::new("redis://127.0.0.1:6379", 3600).await?;
    let embedding = vec![0.1f32; 1536];

    // 쓰기 성능
    let start = Instant::now();
    for i in 0..1000 {
        let text = format!("테스트 텍스트 번호 {}", i);
        cache.set(&text, &embedding, "benchmark").await?;
    }
    let write_time = start.elapsed();

    // 읽기 성능
    let start = Instant::now();
    for i in 0..1000 {
        let text = format!("테스트 텍스트 번호 {}", i);
        let _ = cache.get(&text, "benchmark").await?;
    }
    let read_time = start.elapsed();

    println!(
        "  쓰기: {:?} ({:.2} ops/sec)",
        write_time,
        1000.0 / write_time.as_secs_f64()
    );
    println!(
        "  읽기: {:?} ({:.2} ops/sec)",
        read_time,
        1000.0 / read_time.as_secs_f64()
    );

    cache.clear().await?;
    Ok(())
}

async fn benchmark_compressed_cache() -> Result<()> {
    println!("🗜️  압축 캐시 벤치마크...");

    let cache = CompressedEmbeddingCache::new("redis://127.0.0.1:6379", 3600, 6).await?;
    let embedding = vec![0.1f32; 1536];

    // 쓰기 성능
    let start = Instant::now();
    for i in 0..1000 {
        let text = format!("압축 테스트 텍스트 번호 {}", i);
        cache.set(&text, &embedding, "benchmark").await?;
    }
    let write_time = start.elapsed();

    // 읽기 성능
    let start = Instant::now();
    for i in 0..1000 {
        let text = format!("압축 테스트 텍스트 번호 {}", i);
        let _ = cache.get(&text, "benchmark").await?;
    }
    let read_time = start.elapsed();

    println!(
        "  압축 쓰기: {:?} ({:.2} ops/sec)",
        write_time,
        1000.0 / write_time.as_secs_f64()
    );
    println!(
        "  압축 읽기: {:?} ({:.2} ops/sec)",
        read_time,
        1000.0 / read_time.as_secs_f64()
    );

    // 압축률 확인
    if let Ok(stats) = cache.get_compression_stats().await {
        println!("  압축률: {:.2}x", stats.compression_ratio);
        println!("  메모리 절약: {:.1}%", stats.memory_saved_percent());
    }

    cache.clear_all().await?;
    Ok(())
}

async fn benchmark_concurrent_access() -> Result<()> {
    println!("🚀 동시 접근 벤치마크...");

    let cache = EmbeddingCache::new("redis://127.0.0.1:6379", 3600).await?;
    let embedding = vec![0.1f32; 384];

    // 동시 쓰기 테스트(100 태스크 * 10회 = 1000 ops)
    let start = Instant::now();
    let mut tasks = JoinSet::new();
    for i in 0..100 {
        let cache_clone = EmbeddingCache::new("redis://127.0.0.1:6379", 3600).await?;
        let embedding_clone = embedding.clone();
        tasks.spawn(async move {
            for j in 0..10 {
                let text = format!("동시테스트_{}_{}", i, j);
                let _ = cache_clone.set(&text, &embedding_clone, "concurrent").await;
            }
        });
    }
    while tasks.join_next().await.is_some() {}
    let concurrent_write_time = start.elapsed();

    // 동시 읽기 테스트(1000 ops)
    let start = Instant::now();
    let mut tasks = JoinSet::new();
    for i in 0..100 {
        let cache_clone = EmbeddingCache::new("redis://127.0.0.1:6379", 3600).await?;
        tasks.spawn(async move {
            for j in 0..10 {
                let text = format!("동시테스트_{}_{}", i, j);
                let _ = cache_clone.get(&text, "concurrent").await;
            }
        });
    }
    while tasks.join_next().await.is_some() {}
    let concurrent_read_time = start.elapsed();

    println!("  동시 쓰기 (1000 ops): {:?}", concurrent_write_time);
    println!("  동시 읽기 (1000 ops): {:?}", concurrent_read_time);

    cache.clear().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("⚡ Redis 임베딩 캐시 성능 벤치마크\n====================================");

    benchmark_normal_cache().await?;
    println!();

    benchmark_compressed_cache().await?;
    println!();

    benchmark_concurrent_access().await?;

    println!("\n✅ 모든 벤치마크 완료!");
    Ok(())
}
