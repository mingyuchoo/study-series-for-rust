//! 벡터 검색기 — 코사인 유사도.
//!
//! **도구 이름·스키마·권한 필터는 키워드 검색기와 완전히 같습니다.** 바뀌는
//! 것은 점수를 매기는 방법뿐입니다 — 그것이 "교체 가능하다"의 뜻입니다.
//!
//! 임베딩은 [`Embedder`] 뒤에 있습니다. 기본 구현은 외부 API 없이 도는 해시
//! 기반 임베딩이며(개발·테스트용), 운영에서는 Azure/OpenAI 임베딩으로
//! 교체합니다.

use ai_bridge::adapter::{AllowFn,
                         Chunk,
                         Hit,
                         Query,
                         Retriever};
use anyhow::Result;
use sha2::{Digest,
           Sha256};
use std::sync::{Arc,
                RwLock};

/// 텍스트를 벡터로 바꾸는 것.
#[async_trait::async_trait]
pub trait Embedder: Send + Sync + std::fmt::Debug {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    fn dimensions(&self) -> usize;
}

/// 외부 API 없이 도는 결정적 임베딩 (개발·테스트용).
///
/// 실제 의미를 담지 못하므로 **운영에서 쓰면 안 됩니다.** 벡터 경로가
/// 동작하는지 확인하고 인터페이스를 고정하는 용도입니다.
#[derive(Debug, Clone, Copy)]
pub struct HashEmbedder {
    dims: usize,
}

impl Default for HashEmbedder {
    fn default() -> Self {
        Self {
            dims: 64,
        }
    }
}

#[async_trait::async_trait]
impl Embedder for HashEmbedder {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|t| {
                let mut v = vec![0f32; self.dims];
                for token in t.to_lowercase().split_whitespace() {
                    let mut h = Sha256::new();
                    h.update(token.as_bytes());
                    let d = h.finalize();
                    let idx = (u16::from_be_bytes([d[0], d[1]]) as usize) % self.dims;
                    v[idx] += 1.0;
                }
                normalize(&mut v);
                v
            })
            .collect())
    }

    fn dimensions(&self) -> usize { self.dims }
}

fn normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    // 둘 다 정규화되어 있으므로 내적이 곧 코사인입니다.
    a.iter().zip(b).map(|(x, y)| (x * y) as f64).sum()
}

#[derive(Debug)]
struct Indexed {
    chunk: Chunk,
    embedding: Vec<f32>,
}

#[derive(Debug)]
pub struct VectorRetriever {
    embedder: Arc<dyn Embedder>,
    items: RwLock<Vec<Indexed>>,
}

impl VectorRetriever {
    pub fn new(embedder: Arc<dyn Embedder>) -> Self {
        Self {
            embedder,
            items: RwLock::new(Vec::new()),
        }
    }

    /// 개발·테스트용 (외부 API 없음).
    pub fn in_memory() -> Self { Self::new(Arc::new(HashEmbedder::default())) }
}

#[async_trait::async_trait]
impl Retriever for VectorRetriever {
    async fn index(&self, chunks: Vec<Chunk>) -> Result<()> {
        if chunks.is_empty() {
            return Ok(());
        }
        let texts: Vec<String> = chunks.iter().map(|c| format!("{} {}", c.title, c.text)).collect();
        // 색인 시점에 임베딩을 계산합니다.
        let embeddings = self.embedder.embed(&texts).await?;

        let mut items = self.items.write().unwrap();
        for (chunk, embedding) in chunks.into_iter().zip(embeddings) {
            items.push(Indexed {
                chunk,
                embedding,
            });
        }
        Ok(())
    }

    async fn search(&self, q: &Query, allow: AllowFn<'_>) -> Result<Vec<Hit>> {
        let top_k = if q.top_k == 0 { 5 } else { q.top_k };
        // 검색기가 죽으면 빈 결과가 아니라 **오류**를 올립니다 — 빈 결과를 돌려주면
        // LLM 은 "그런 규정이 없다"고 답합니다.
        let qv = self.embedder.embed(std::slice::from_ref(&q.text)).await?.into_iter().next().unwrap_or_default();

        let items = self.items.read().unwrap();
        let mut hits: Vec<Hit> = items
            .iter()
            // **권한 필터가 점수 매기기 전에.** 키워드 검색기와 같은 자리입니다.
            .filter(|i| allow(&i.chunk))
            .map(|i| Hit {
                chunk: i.chunk.clone(),
                score: cosine(&qv, &i.embedding),
            })
            .filter(|h| h.score > 0.0)
            .collect();

        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(top_k);
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: &str, title: &str, text: &str, roles: &[&str]) -> Chunk {
        Chunk {
            doc_id: id.into(),
            chunk_id: format!("{id}#1"),
            title: title.into(),
            text: text.into(),
            roles: roles.iter().map(|s| s.to_string()).collect(),
            customer_id: String::new(),
            uri: format!("docs://{id}"),
        }
    }

    #[tokio::test]
    async fn finds_semantically_indexed_chunks() {
        let r = VectorRetriever::in_memory();
        r.index(vec![
            chunk("DOC-1", "휴가 규정", "연차는 매년 15일 부여됩니다", &[]),
            chunk("DOC-2", "보안 규정", "비밀번호는 90일마다 변경합니다", &[]),
        ])
        .await
        .unwrap();

        let hits = r
            .search(
                &Query {
                    text: "연차는".into(),
                    top_k: 5,
                },
                &|_| true,
            )
            .await
            .unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].chunk.doc_id, "DOC-1");
    }

    #[tokio::test]
    async fn permission_filter_runs_before_scoring_here_too() {
        // 검색기를 갈아끼워도 권한 필터의 위치는 그대로입니다.
        let r = VectorRetriever::in_memory();
        r.index(vec![chunk("DOC-3", "임원 보상", "임원 성과급은 연봉의 30% 이내입니다", &["hr"])])
            .await
            .unwrap();

        let hits = r
            .search(
                &Query {
                    text: "성과급은".into(),
                    top_k: 5,
                },
                &|c: &Chunk| c.roles.is_empty(),
            )
            .await
            .unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn embeddings_are_normalized() {
        let e = HashEmbedder::default();
        let v = e.embed(&["hello world".to_string()]).await.unwrap();
        let norm: f32 = v[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }
}
