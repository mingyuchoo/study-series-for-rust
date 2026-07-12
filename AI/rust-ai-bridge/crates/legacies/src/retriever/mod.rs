//! RAG 검색기 — 교체 가능합니다.
//!
//! 문서를 그대로 검색하지 않고 **조각(Chunk)으로 나눠 색인**합니다. 긴 문서에서
//! 질문과 관련된 부분만 LLM에게 주어야 컨텍스트가 낭비되지 않고, 근거 링크가
//! 문서가 아니라 문단을 가리켜야 사용자가 확인할 수 있습니다.
//!
//! 교체되는 것은 [`Retriever`](ai_bridge::adapter::Retriever) 하나입니다.
//! pgvector 구현을 넘기면 검색이 코사인 유사도로 바뀌지만, **도구
//! 이름·스키마·권한 필터는 그대로입니다.**

mod keyword;
mod vector;

use ai_bridge::adapter::Chunk;
pub use keyword::KeywordRetriever;
pub use vector::VectorRetriever;

/// 문서를 문장 경계를 지키며 조각으로 나눕니다.
///
/// 문장 중간에서 자르면 근거로 보여줄 때 말이 잘려 사용자가 확인할 수 없습니다.
pub fn chunk_document(doc_id: &str, title: &str, text: &str, roles: &[String], customer_id: &str, max_chars: usize) -> Vec<Chunk> {
    let max_chars = if max_chars == 0 { 400 } else { max_chars };
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut n = 0;

    let mut flush = |current: &mut String, n: &mut usize| {
        let body = current.trim();
        if body.is_empty() {
            return;
        }
        *n += 1;
        chunks.push(Chunk {
            doc_id: doc_id.to_string(),
            chunk_id: format!("{doc_id}#{n}"),
            title: title.to_string(),
            text: body.to_string(),
            roles: roles.to_vec(),
            customer_id: customer_id.to_string(),
            uri: format!("docs://{doc_id}"),
        });
        current.clear();
    };

    for sentence in split_sentences(text) {
        if !current.is_empty() && current.chars().count() + sentence.chars().count() > max_chars {
            flush(&mut current, &mut n);
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(sentence.trim());
    }
    flush(&mut current, &mut n);
    chunks
}

/// 문장 경계로 나눕니다 (한국어의 `다.` 와 영어의 `.`/`!`/`?`).
fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        cur.push(c);
        if matches!(c, '.' | '!' | '?' | '\n') {
            let t = cur.trim();
            if !t.is_empty() {
                out.push(t.to_string());
            }
            cur.clear();
        }
    }
    let t = cur.trim();
    if !t.is_empty() {
        out.push(t.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_keep_sentence_boundaries() {
        let text = "첫 문장입니다. 두 번째 문장입니다. 세 번째 문장입니다.";
        let cs = chunk_document("DOC-1", "제목", text, &[], "", 20);
        assert!(cs.len() > 1);
        // 어느 조각도 문장 중간에서 잘리지 않아야 합니다.
        for c in &cs {
            assert!(c.text.ends_with('.'), "문장 중간에서 잘렸습니다: {}", c.text);
        }
    }

    #[test]
    fn chunk_ids_and_uris_are_stable() {
        let cs = chunk_document("DOC-1", "제목", "한 문장.", &[], "", 400);
        assert_eq!(cs[0].chunk_id, "DOC-1#1");
        assert_eq!(cs[0].uri, "docs://DOC-1");
    }

    #[test]
    fn empty_text_yields_no_chunks() {
        assert!(chunk_document("DOC-1", "제목", "   ", &[], "", 400).is_empty());
    }
}
