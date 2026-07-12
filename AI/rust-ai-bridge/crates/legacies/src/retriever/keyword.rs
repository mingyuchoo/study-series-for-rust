//! 키워드 검색기 (기본).
//!
//! **권한 필터는 점수를 매기기 전에 적용됩니다.** 필터를 검색 뒤에 걸면 상위
//! K건이 전부 걸러져 빈 결과가 나옵니다 — 그러면 LLM 은 "그런 규정이 없다"고
//! 답합니다.

use ai_bridge::adapter::{AllowFn,
                         Chunk,
                         Hit,
                         Query,
                         Retriever};
use anyhow::Result;
use std::sync::RwLock;

#[derive(Debug, Default)]
pub struct KeywordRetriever {
    chunks: RwLock<Vec<Chunk>>,
}

impl KeywordRetriever {
    pub fn new() -> Self { Self::default() }
}

#[async_trait::async_trait]
impl Retriever for KeywordRetriever {
    async fn index(&self, chunks: Vec<Chunk>) -> Result<()> {
        self.chunks.write().unwrap().extend(chunks);
        Ok(())
    }

    async fn search(&self, q: &Query, allow: AllowFn<'_>) -> Result<Vec<Hit>> {
        let top_k = if q.top_k == 0 { 5 } else { q.top_k };
        let terms: Vec<String> = q.text.to_lowercase().split_whitespace().map(|s| s.to_string()).collect();

        let chunks = self.chunks.read().unwrap();
        let mut hits: Vec<Hit> = chunks
            .iter()
            // **권한 필터가 먼저.** 볼 수 없는 조각은 점수 경쟁에 아예 참여하지 않습니다.
            .filter(|c| allow(c))
            .filter_map(|c| {
                let score = score(c, &terms);
                if score > 0.0 {
                    Some(Hit {
                        chunk: c.clone(),
                        score,
                    })
                } else {
                    None
                }
            })
            .collect();

        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(top_k);
        Ok(hits)
    }
}

/// 아주 단순한 용어 빈도 점수. 제목 일치에 가중치를 둡니다.
fn score(c: &Chunk, terms: &[String]) -> f64 {
    if terms.is_empty() {
        // 질의가 비면 모든 조각이 같은 점수로 후보가 됩니다.
        return 0.1;
    }
    let text = c.text.to_lowercase();
    let title = c.title.to_lowercase();
    let mut s = 0.0;
    for t in terms {
        if text.contains(t) {
            s += 1.0;
        }
        if title.contains(t) {
            s += 0.5;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: &str, title: &str, text: &str, roles: &[&str], customer: &str) -> Chunk {
        Chunk {
            doc_id: id.into(),
            chunk_id: format!("{id}#1"),
            title: title.into(),
            text: text.into(),
            roles: roles.iter().map(|s| s.to_string()).collect(),
            customer_id: customer.into(),
            uri: format!("docs://{id}"),
        }
    }

    async fn retriever() -> KeywordRetriever {
        let r = KeywordRetriever::new();
        r.index(vec![
            chunk("DOC-1", "휴가 규정", "연차는 매년 15일 부여됩니다.", &[], ""),
            chunk("DOC-2", "보안 규정", "비밀번호는 90일마다 변경합니다.", &[], ""),
            chunk("DOC-3", "임원 보상 규정", "임원 성과급은 연봉의 30% 이내입니다.", &["hr", "admin"], ""),
        ])
        .await
        .unwrap();
        r
    }

    #[tokio::test]
    async fn finds_matching_chunks() {
        let r = retriever().await;
        let hits = r
            .search(
                &Query {
                    text: "연차".into(),
                    top_k: 5,
                },
                &|_| true,
            )
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk.doc_id, "DOC-1");
    }

    #[tokio::test]
    async fn permission_filter_runs_before_scoring() {
        // 볼 수 없는 문서는 상위 K 경쟁에 참여조차 하지 않습니다.
        let r = retriever().await;
        let hits = r
            .search(
                &Query {
                    text: "성과급".into(),
                    top_k: 5,
                },
                // hr 역할이 아닌 주체.
                &|c: &Chunk| c.roles.is_empty(),
            )
            .await
            .unwrap();
        assert!(hits.is_empty(), "볼 수 없는 문서가 검색 결과에 나왔습니다");

        // hr 이면 보입니다.
        let hits = r
            .search(
                &Query {
                    text: "성과급".into(),
                    top_k: 5,
                },
                &|c: &Chunk| c.roles.is_empty() || c.roles.iter().any(|r| r == "hr"),
            )
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[tokio::test]
    async fn top_k_limits_results() {
        let r = retriever().await;
        let hits = r
            .search(
                &Query {
                    text: "규정".into(),
                    top_k: 1,
                },
                &|_| true,
            )
            .await
            .unwrap();
        assert!(hits.len() <= 1);
    }

    #[tokio::test]
    async fn hits_are_sorted_by_score() {
        let r = KeywordRetriever::new();
        r.index(vec![chunk("A", "보안", "보안 보안 보안", &[], ""), chunk("B", "기타", "보안", &[], "")])
            .await
            .unwrap();
        let hits = r
            .search(
                &Query {
                    text: "보안".into(),
                    top_k: 5,
                },
                &|_| true,
            )
            .await
            .unwrap();
        assert_eq!(hits[0].chunk.doc_id, "A");
        assert!(hits[0].score >= hits[1].score);
    }
}
