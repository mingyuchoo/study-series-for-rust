// 압축 임베딩 캐시
use anyhow::Result;
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use redis::{AsyncCommands, aio::ConnectionManager};
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{CacheError, CacheStats, EmbeddingVector};

pub struct CompressedEmbeddingCache {
    connection: Arc<RwLock<ConnectionManager>>,
    ttl: u64,
    stats: Arc<RwLock<CacheStats>>,
    compression_level: u32,
}

impl CompressedEmbeddingCache {
    pub async fn new(redis_url: &str, ttl: u64, compression_level: u32) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let connection = client.get_connection_manager().await?;
        Ok(Self {
            connection: Arc::new(RwLock::new(connection)),
            ttl,
            stats: Arc::new(RwLock::new(CacheStats {
                hit_count: 0,
                miss_count: 0,
                total_keys: 0,
            })),
            compression_level,
        })
    }

    fn make_key(&self, text: &str, model: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(format!("{}:{}", model, text).as_bytes());
        let hash = hex::encode(hasher.finalize());
        format!("cemb:{}", &hash[..12])
    }

    fn compress_embedding(&self, embedding: &EmbeddingVector) -> Result<Vec<u8>> {
        // f32 -> LE 바이트 배열
        let bytes: Vec<u8> = embedding
            .iter()
            .flat_map(|&f| f.to_le_bytes().to_vec())
            .collect();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::new(self.compression_level));
        encoder.write_all(&bytes)?;
        Ok(encoder.finish()?)
    }

    fn decompress_embedding(&self, compressed: &[u8]) -> Result<EmbeddingVector> {
        let mut decoder = GzDecoder::new(compressed);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        let embedding: EmbeddingVector = decompressed
            .chunks(4)
            .map(|chunk| {
                let bytes = [chunk[0], chunk[1], chunk[2], chunk[3]];
                f32::from_le_bytes(bytes)
            })
            .collect();
        Ok(embedding)
    }

    pub async fn get(
        &self,
        text: &str,
        model: &str,
    ) -> Result<Option<EmbeddingVector>, CacheError> {
        let key = self.make_key(text, model);
        let mut conn = self.connection.write().await;
        let data_opt: Option<Vec<u8>> = conn.get(&key).await?;
        if let Some(compressed_data) = data_opt {
            let embedding = self.decompress_embedding(&compressed_data).map_err(|_| {
                CacheError::Serialization(bincode::Error::new(bincode::ErrorKind::Custom(
                    "Decompression failed".to_string(),
                )))
            })?;
            let mut stats = self.stats.write().await;
            stats.hit_count += 1;
            Ok(Some(embedding))
        } else {
            let mut stats = self.stats.write().await;
            stats.miss_count += 1;
            Ok(None)
        }
    }

    pub async fn set(
        &self,
        text: &str,
        embedding: &EmbeddingVector,
        model: &str,
    ) -> Result<(), CacheError> {
        let key = self.make_key(text, model);
        let compressed_data = self.compress_embedding(embedding).map_err(|_| {
            CacheError::Serialization(bincode::Error::new(bincode::ErrorKind::Custom(
                "Compression failed".to_string(),
            )))
        })?;
        let mut conn = self.connection.write().await;
        conn.set_ex::<_, _, ()>(&key, &compressed_data, self.ttl)
            .await?;
        Ok(())
    }

    pub async fn get_compression_stats(&self) -> Result<CompressionStats> {
        let mut conn = self.connection.write().await;
        let script = redis::Script::new(
            r#"
            local keys = redis.call('KEYS', 'cemb:*')
            local sample = {}
            for i=1,math.min(10, #keys) do table.insert(sample, keys[i]) end
            return sample
            "#,
        );
        let keys: Vec<String> = script.invoke_async(&mut *conn).await.unwrap_or_default();
        let mut total_compressed = 0usize;
        let mut sample_count = 0usize;
        for key in keys {
            if let Ok(data) = conn.get::<_, Vec<u8>>(&key).await {
                total_compressed += data.len();
                sample_count += 1;
            }
        }
        let avg_compressed_size = total_compressed.checked_div(sample_count).unwrap_or(0);
        let uncompressed_size = 1536 * 4; // OpenAI/Azure 기본 임베딩(예시) 1536차원
        let compression_ratio = if avg_compressed_size > 0 {
            uncompressed_size as f64 / avg_compressed_size as f64
        } else {
            1.0
        };
        Ok(CompressionStats {
            avg_compressed_size,
            estimated_uncompressed_size: uncompressed_size,
            compression_ratio,
            sample_count,
        })
    }

    /// cemb:* 패턴의 모든 키 삭제
    pub async fn clear_all(&self) -> Result<()> {
        let mut conn = self.connection.write().await;
        let script = redis::Script::new(
            r#"
            local keys = redis.call('KEYS', 'cemb:*')
            for i=1,#keys do redis.call('DEL', keys[i]) end
            return #keys
            "#,
        );
        let _: () = script.invoke_async(&mut *conn).await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct CompressionStats {
    pub avg_compressed_size: usize,
    pub estimated_uncompressed_size: usize,
    pub compression_ratio: f64,
    pub sample_count: usize,
}

impl CompressionStats {
    pub fn memory_saved_percent(&self) -> f64 {
        (1.0 - (self.avg_compressed_size as f64 / self.estimated_uncompressed_size as f64)) * 100.0
    }
}
