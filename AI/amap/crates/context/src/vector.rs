//! Semantic retrieval boundary. The in-memory implementation uses hashed bag-of-words vectors
//! as a stand-in embedding; production swaps in pgvector (see `migrations/`) or Qdrant.
use crate::Hit;
use std::collections::HashMap;

pub trait VectorIndex: Send + Sync {
    fn add(&mut self, id: &str, kind: &str, text: &str);
    fn search(&self, query: &str, limit: usize) -> Vec<Hit>;
}

const DIM: usize = 512;

fn embed(text: &str) -> Vec<f32> {
    let mut v = vec![0f32; DIM];
    for tok in text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|t| t.len() > 1) {
        let mut h: u64 = 1469598103934665603;
        for b in tok.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(1099511628211);
        }
        v[(h % DIM as u64) as usize] += 1.0;
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter_mut().for_each(|x| *x /= norm);
    }
    v
}

#[derive(Default)]
pub struct InMemoryVectorIndex {
    rows: HashMap<String, (String, Vec<f32>)>,
}

impl VectorIndex for InMemoryVectorIndex {
    fn add(&mut self, id: &str, kind: &str, text: &str) {
        self.rows.insert(id.to_string(), (kind.to_string(), embed(text)));
    }
    fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let q = embed(query);
        let mut hits: Vec<Hit> = self
            .rows
            .iter()
            .map(|(id, (kind, v))| Hit { id: id.clone(), kind: kind.clone(), score: v.iter().zip(&q).map(|(a, b)| a * b).sum() })
            .filter(|h| h.score > 0.0)
            .collect();
        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        hits.truncate(limit);
        hits
    }
}
