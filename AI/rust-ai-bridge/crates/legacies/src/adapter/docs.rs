//! 사내 문서 검색 (RAG).
//!
//! **권한 기반 검색.** `search_documents` 는 두 기준으로 검색 대상을 먼저
//! 좁힙니다:
//!
//! - **역할 기반** — 임원 보상 문서는 `hr`/`admin` 역할만 봅니다.
//! - **속성 기반** — 고객별 계약 메모는 그 고객의 담당자(`managed_customers`)만
//!   봅니다.
//!
//! 볼 수 없는 문서는 "권한이 없습니다"가 아니라 **아예 존재하지 않는 것처럼**
//! 처리합니다 — 결과 건수나 제목이 새어나가는 것 자체가 정보 노출이기
//! 때문입니다.
//!
//! 문서는 MCP **Resource** 로도 노출됩니다(`docs://DOC-001`). 검색 결과의 근거
//! 링크가 가리키는 대상을 LLM 이 도구 호출 없이 읽을 수 있고, **자원 읽기도
//! 같은 행 수준 접근 제어를 통과합니다.**

use crate::retriever::{KeywordRetriever,
                       chunk_document};
use ai_bridge::{adapter::{Adapter,
                          Chunk,
                          Query,
                          Resource,
                          ResourceReader,
                          Retriever,
                          array_prop,
                          int_prop,
                          num_prop,
                          object,
                          str_prop},
                auth::Identity,
                registry::{Access,
                           RiskLevel,
                           Sensitivity,
                           Spec,
                           Tool,
                           handler}};
use anyhow::Result;
use serde_json::{Map,
                 Value,
                 json};
use std::sync::Arc;

/// 사내 문서 한 건.
struct Doc {
    id: &'static str,
    title: &'static str,
    text: &'static str,
    /// 볼 수 있는 역할. 비면 전사 공개.
    roles: &'static [&'static str],
    /// 이 고객의 담당자만 볼 수 있음. 비면 고객 제한 없음.
    customer_id: &'static str,
}

fn mock_docs() -> Vec<Doc> {
    vec![
        Doc {
            id: "DOC-001",
            title: "연차 휴가 규정",
            text: "연차는 매년 15일 부여됩니다. 근속 3년마다 1일씩 가산되며 최대 25일입니다. \
                   미사용 연차는 다음 해로 이월되지 않습니다.",
            roles: &[],
            customer_id: "",
        },
        Doc {
            id: "DOC-002",
            title: "정보보안 규정",
            text: "비밀번호는 90일마다 변경해야 합니다. 사내 시스템 접근은 사내망 또는 VPN 을 \
                   통해서만 허용됩니다. 외부 저장소에 고객 정보를 업로드할 수 없습니다.",
            roles: &[],
            customer_id: "",
        },
        Doc {
            id: "DOC-003",
            title: "임원 보상 규정",
            // 인사 문서 — hr/admin 만 볼 수 있습니다.
            text: "임원 성과급은 연봉의 30% 이내에서 이사회 결의로 정합니다. \
                   스톡옵션은 재임 2년 이후 행사할 수 있습니다.",
            roles: &["hr", "admin"],
            customer_id: "",
        },
        Doc {
            id: "DOC-004",
            title: "CUST-1001 계약 협상 메모",
            // 고객별 메모 — 그 고객의 담당자만 볼 수 있습니다.
            text: "클라우드 유지보수 계약 갱신 시 15% 할인을 요구했습니다. \
                   경쟁사 견적을 근거로 제시했습니다.",
            roles: &[],
            customer_id: "CUST-1001",
        },
        Doc {
            id: "DOC-005",
            title: "환불 정책",
            text: "결제 완료 후 30일 이내에만 환불이 가능합니다. \
                   환불은 원 결제 수단으로만 처리됩니다.",
            roles: &[],
            customer_id: "",
        },
    ]
}

/// 이 주체가 이 조각을 볼 수 있는가.
///
/// **이 술어가 검색기 밖에 있고 점수 매기기 전에 적용됩니다.**
fn can_see(id: &Identity, c: &Chunk) -> bool {
    // 역할 기반.
    if !c.roles.is_empty() && !c.roles.iter().any(|r| id.has_role(r)) {
        return false;
    }
    // 속성 기반 — 고객별 문서는 담당자만.
    if !c.customer_id.is_empty() {
        if id.has_role("admin") {
            return true;
        }
        let managed = id
            .attr("managed_customers")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()).any(|m| m == c.customer_id))
            .unwrap_or(false);
        return managed;
    }
    true
}

pub struct DocsAdapter {
    retriever: Arc<dyn Retriever>,
}

impl DocsAdapter {
    /// 검색기를 받아 색인까지 마칩니다.
    pub async fn new(retriever: Arc<dyn Retriever>) -> Result<Self> {
        let mut chunks = Vec::new();
        for d in mock_docs() {
            let roles: Vec<String> = d.roles.iter().map(|s| s.to_string()).collect();
            chunks.extend(chunk_document(d.id, d.title, d.text, &roles, d.customer_id, 400));
        }
        retriever.index(chunks).await?;
        Ok(Self {
            retriever,
        })
    }

    pub async fn in_memory() -> Result<Self> { Self::new(Arc::new(KeywordRetriever::new())).await }
}

struct DocReader {
    doc_id: String,
}

#[async_trait::async_trait]
impl ResourceReader for DocReader {
    async fn read(&self, id: &Identity) -> Result<String> {
        let docs = mock_docs();
        let d = docs
            .iter()
            .find(|d| d.id == self.doc_id)
            .ok_or_else(|| anyhow::anyhow!("문서 {} 을(를) 찾을 수 없습니다", self.doc_id))?;

        // **자원 읽기도 도구와 동일한 행 수준 접근 제어를 통과합니다.**
        let probe = Chunk {
            roles: d.roles.iter().map(|s| s.to_string()).collect(),
            customer_id: d.customer_id.to_string(),
            ..Default::default()
        };
        if !can_see(id, &probe) {
            // 존재 자체를 알리지 않습니다.
            return Err(anyhow::anyhow!("문서 {} 을(를) 찾을 수 없습니다", self.doc_id));
        }
        Ok(format!("# {}\n\n{}", d.title, d.text))
    }
}

#[async_trait::async_trait]
impl Adapter for DocsAdapter {
    fn name(&self) -> String { "docs".into() }

    async fn health_check(&self) -> Result<()> { Ok(()) }

    fn resources(&self) -> Vec<Resource> {
        mock_docs()
            .into_iter()
            .map(|d| Resource {
                uri: format!("docs://{}", d.id),
                name: d.title.to_string(),
                description: format!("사내 문서 {}", d.id),
                mime_type: "text/markdown".into(),
                read: Arc::new(DocReader {
                    doc_id: d.id.to_string(),
                }),
            })
            .collect()
    }

    fn tools(&self) -> Vec<Tool> {
        let r = self.retriever.clone();

        vec![Tool {
            spec: Spec {
                name: "search_documents".into(),
                description: "사내 문서를 검색합니다. 권한이 있는 문서만 검색됩니다.".into(),
                system: "docs".into(),
                access: Access::Read,
                risk_level: RiskLevel::L1,
                sensitivity: Sensitivity::Internal,
                required_permissions: vec!["docs.read".into()],
                rate_limit_per_min: 60,
                timeout_ms: 5_000,
                max_retries: 2,
                log_retention_days: 90,
                fallback: "IT운영팀(it-ops@example.com)에 문의하세요.".into(),
                input_schema: object(
                    vec![("query", str_prop("검색할 내용")), ("top_k", int_prop("최대 결과 수 (기본 5)"))],
                    &["query"],
                ),
                output_schema: object(
                    vec![
                        (
                            "results",
                            array_prop(
                                "검색 결과",
                                object(
                                    vec![
                                        ("doc_id", str_prop("문서 ID")),
                                        ("title", str_prop("제목")),
                                        ("snippet", str_prop("본문 조각")),
                                        ("uri", str_prop("근거 링크. 예: docs://DOC-001")),
                                        ("score", num_prop("관련도")),
                                    ],
                                    &["doc_id", "title", "snippet", "uri"],
                                ),
                            ),
                        ),
                        ("truncated", json!({"type":"boolean"})),
                        ("truncated_reason", str_prop("잘린 이유")),
                    ],
                    &["results"],
                ),
                ..Default::default()
            },
            handler: handler(move |id: Identity, args: Map<String, Value>| {
                let r = r.clone();
                async move {
                    let q = Query {
                        text: args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        top_k: args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(5) as usize,
                    };
                    // 권한 술어를 검색기에 **넘깁니다** — 검색기 안에 두지 않습니다.
                    let hits = r.search(&q, &|c: &Chunk| can_see(&id, c)).await?;

                    // 조각을 문서 단위로 묶어 근거 링크를 붙입니다.
                    let mut results: Vec<Value> = Vec::new();
                    let mut seen: Vec<String> = Vec::new();
                    for h in hits {
                        if seen.contains(&h.chunk.doc_id) {
                            continue;
                        }
                        seen.push(h.chunk.doc_id.clone());
                        results.push(json!({
                            "doc_id": h.chunk.doc_id,
                            "title": h.chunk.title,
                            "snippet": h.chunk.text,
                            "uri": h.chunk.uri,
                            "score": h.score,
                        }));
                    }
                    Ok(json!({ "results": results }))
                }
            }),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn args(v: Value) -> Map<String, Value> { v.as_object().unwrap().clone() }

    fn identity(roles: &[&str], managed: &[&str]) -> Identity {
        Identity {
            user_id: "u".into(),
            roles: roles.iter().map(|s| s.to_string()).collect(),
            attributes: HashMap::from([("managed_customers".to_string(), json!(managed))]),
            ..Default::default()
        }
    }

    async fn search_tool() -> Tool { DocsAdapter::in_memory().await.unwrap().tools().into_iter().next().unwrap() }

    #[tokio::test]
    async fn finds_public_documents() {
        let t = search_tool().await;
        let out = t.handler.call(&identity(&["sales"], &[]), &args(json!({"query":"연차"}))).await.unwrap();
        let results = out["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["doc_id"], json!("DOC-001"));
        // 근거 링크는 MCP Resource URI 를 가리킵니다.
        assert_eq!(results[0]["uri"], json!("docs://DOC-001"));
    }

    #[tokio::test]
    async fn role_restricted_documents_are_invisible_not_refused() {
        let t = search_tool().await;

        // 영업팀에게 임원 보상 문서는 **존재하지 않습니다.**
        let out = t.handler.call(&identity(&["sales"], &[]), &args(json!({"query":"성과급"}))).await.unwrap();
        assert!(out["results"].as_array().unwrap().is_empty(), "볼 수 없는 문서의 존재가 드러났습니다");

        // 인사팀에게는 보입니다.
        let out = t.handler.call(&identity(&["hr"], &[]), &args(json!({"query":"성과급"}))).await.unwrap();
        assert_eq!(out["results"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn customer_notes_are_visible_only_to_the_account_manager() {
        let t = search_tool().await;

        let out = t
            .handler
            .call(&identity(&["sales"], &["CUST-1001"]), &args(json!({"query":"할인"})))
            .await
            .unwrap();
        assert_eq!(out["results"].as_array().unwrap().len(), 1);

        // 담당이 아닌 영업사원에게는 보이지 않습니다.
        let out = t
            .handler
            .call(&identity(&["sales"], &["CUST-9999"]), &args(json!({"query":"할인"})))
            .await
            .unwrap();
        assert!(out["results"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn resources_enforce_the_same_row_level_access_control() {
        let a = DocsAdapter::in_memory().await.unwrap();
        let rs = a.resources();
        let exec = rs.iter().find(|r| r.uri == "docs://DOC-003").unwrap();

        // 영업팀은 임원 보상 문서를 자원으로도 읽을 수 없습니다.
        assert!(exec.read.read(&identity(&["sales"], &[])).await.is_err());
        // 인사팀은 읽을 수 있습니다.
        let body = exec.read.read(&identity(&["hr"], &[])).await.unwrap();
        assert!(body.contains("성과급"));
    }

    #[tokio::test]
    async fn the_vector_retriever_enforces_the_same_filter() {
        // 검색기를 갈아끼워도 권한 필터는 그대로입니다.
        //
        // 단언은 "결과가 비었다"가 아니라 **"제한된 문서가 없다"** 입니다. 개발용
        // 해시 임베딩은 해시 충돌로 무관한 문서에도 0이 아닌 점수를 줄 수 있으므로,
        // 빈 결과를 요구하면 권한과 무관한 이유로 실패합니다.
        use crate::retriever::VectorRetriever;
        let a = DocsAdapter::new(Arc::new(VectorRetriever::in_memory())).await.unwrap();
        let t = a.tools().into_iter().next().unwrap();

        let out = t.handler.call(&identity(&["sales"], &[]), &args(json!({"query":"성과급은"}))).await.unwrap();
        let ids: Vec<&str> = out["results"].as_array().unwrap().iter().map(|r| r["doc_id"].as_str().unwrap()).collect();
        assert!(!ids.contains(&"DOC-003"), "검색기를 바꿨더니 볼 수 없는 문서가 새어나왔습니다: {ids:?}");

        // 인사팀에게는 여전히 보입니다 — 필터가 과하게 막지도 않습니다.
        let out = t.handler.call(&identity(&["hr"], &[]), &args(json!({"query":"성과급은"}))).await.unwrap();
        let ids: Vec<&str> = out["results"].as_array().unwrap().iter().map(|r| r["doc_id"].as_str().unwrap()).collect();
        assert!(ids.contains(&"DOC-003"));
    }
}
