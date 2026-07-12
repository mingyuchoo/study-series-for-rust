//! 운영 콘솔.
//!
//! # 콘솔은 인증 없이 띄울 수 없습니다
//!
//! 이 화면은 **프롬프트 원문과 도구 입출력**을 보여줍니다. 인증 없는 관리자
//! 콘솔은 없는 것보다 나쁩니다. [`Console::new`] 는 리졸버가 없으면 오류를
//! 냅니다 — 실수로 열어둘 방법이 없습니다.
//!
//! # 헤더 인증은 CSRF 를 막지 못합니다
//!
//! 주체가 프록시가 붙이는 헤더로 결정되므로, **다른 사이트의 폼에서 온 요청에도
//! 브라우저가 그 헤더를 실어 보냅니다.** 관리자가 악성 링크를 한 번 누르면 승인
//! 버튼이 눌린 것과 같습니다. 그래서 POST 에는 동일 출처 검사를 겁니다 — 이
//! 저장소에는 CSRF **토큰이 없으며**, 브라우저가 붙이는 헤더가 유일한
//! 방어입니다.
//!
//! # 결정자는 폼이 아니라 세션에서 옵니다
//!
//! 승인/거부 버튼은 `by` 를 받지 않습니다. **콘솔에 로그인한 관리자가 결정자로
//! 기록됩니다.** 폼으로 받으면 아무 이름이나 적어 넣을 수 있고 감사 추적이
//! 거짓이 됩니다.
//!
//! # 자동 이스케이프
//!
//! 감사 로그에는 LLM 이 만든 문자열과 사용자 프롬프트가 그대로 들어 있습니다.
//! `maud` 가 자동 이스케이프하지 않으면 **프롬프트 인젝션이 관리자 브라우저까지
//! 이어집니다.**

mod pages;
mod templates;

use crate::{auth::{Identity,
                   RequestContext,
                   SharedResolver},
            ops::Service};
use axum::{Router,
           extract::{Request,
                     State},
           http::{HeaderMap,
                  StatusCode,
                  Uri,
                  header},
           middleware::{self,
                        Next},
           response::{IntoResponse,
                      Response},
           routing::{get,
                     post}};
use std::sync::Arc;

/// 콘솔 접근에 요구하는 기본 역할.
pub const ADMIN_ROLE: &str = "admin";

/// 콘솔 상태.
#[derive(Clone)]
pub struct Console {
    pub(crate) ops: Arc<Service>,
    pub(crate) resolve: SharedResolver,
    pub(crate) role: String,
}

/// 현재 로그인한 관리자. **결정자는 여기서 옵니다 — 폼이 아니라.**
#[derive(Debug, Clone)]
pub struct Viewer(pub Identity);

impl Console {
    /// 콘솔을 만듭니다.
    ///
    /// **리졸버가 없으면 오류입니다** — 인증 없는 콘솔을 실수로 열어둘 방법을
    /// 남기지 않기 위함입니다.
    pub fn new(ops: Arc<Service>, resolve: Option<SharedResolver>, role: &str) -> anyhow::Result<Self> {
        let Some(resolve) = resolve else {
            anyhow::bail!("console: auth resolver is required");
        };
        Ok(Self {
            ops,
            resolve,
            role: if role.is_empty() { ADMIN_ROLE.to_string() } else { role.to_string() },
        })
    }

    /// 라우터를 만듭니다.
    ///
    /// **프로브는 인증 미들웨어 밖에 있습니다** — K8s 가 토큰 없이 부를 수
    /// 있어야 합니다.
    pub fn router(self) -> Router {
        let authed = Router::new()
            .route("/", get(pages::dashboard))
            .route("/calls", get(pages::calls))
            .route("/injection", get(pages::injection))
            .route("/stats", get(pages::stats))
            .route("/approvals", get(pages::approvals))
            .route("/approvals/{id}/decide", post(pages::decide))
            .route("/workflows", get(pages::workflows))
            .route("/retention", get(pages::retention))
            .route("/health", get(pages::health))
            .route("/inventory", get(pages::inventory))
            .route("/tools", get(pages::tools).post(pages::tools_post))
            .route("/agents", get(pages::agents))
            .route("/agents/register", post(pages::register_agent))
            .layer(middleware::from_fn_with_state(self.clone(), authenticate))
            .with_state(self.clone());

        // 프로브는 인증보다 **먼저** 매칭됩니다.
        Router::new()
            .route("/livez", get(pages::livez))
            .route("/readyz", get(pages::readyz))
            .route("/healthz", get(pages::livez))
            .with_state(self)
            .merge(authed)
    }
}

/// 인증 미들웨어.
///
/// 순서가 중요합니다:
/// 1. **POST 면 동일 출처 검사 먼저** — 인증되지 않은 교차 출처 POST 도 주체
///    해석 전에 막습니다.
/// 2. 401 — 주체를 해석하지 못함.
/// 3. 403 — 역할 부족.
// 반환은 `Response` 하나입니다. `Result<Response, Response>` 로 하면 clippy 의
// `result_large_err` 에 걸리고, 어차피 양쪽이 같은 타입이라 의미도 없습니다.
async fn authenticate(State(c): State<Console>, mut req: Request, next: Next) -> Response {
    // 1. CSRF — 헤더 인증은 교차 출처 요청을 막지 못합니다.
    if req.method() == axum::http::Method::POST && !same_origin(req.headers(), req.uri()) {
        return (StatusCode::FORBIDDEN, "교차 출처 요청은 허용되지 않습니다").into_response();
    }

    // 실제 피어 주소를 심습니다 — 클라이언트가 위조할 수 없습니다.
    let peer = req
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|c| c.0.to_string())
        .unwrap_or_default();

    let mut rc = RequestContext {
        now: Some(chrono::Utc::now()),
        ..Default::default()
    };
    for (k, v) in req.headers().iter() {
        if let Ok(s) = v.to_str() {
            rc.set(k.as_str(), s);
        }
    }
    if !peer.is_empty() {
        rc.set(crate::auth::REMOTE_ADDR_HEADER, &peer);
    }

    // 2. 인증.
    let id = match c.resolve.resolve(&rc) {
        | Ok(id) => id,
        | Err(e) => {
            return (StatusCode::UNAUTHORIZED, format!("인증되지 않은 요청입니다: {e}")).into_response();
        },
    };

    // 3. 인가.
    if !id.has_role(&c.role) {
        return (StatusCode::FORBIDDEN, format!("{:?} 역할이 필요합니다", c.role)).into_response();
    }

    req.extensions_mut().insert(Viewer(id));
    next.run(req).await
}

/// 동일 출처 검사.
///
/// **`Sec-Fetch-Site` 를 먼저 봅니다** — 브라우저가 붙이며 위조할 수 없습니다.
/// 없으면 `Origin` 을 `Host` 와 대조합니다. **둘 다 없는 POST 는 거부합니다.**
fn same_origin(headers: &HeaderMap, _uri: &Uri) -> bool {
    if let Some(site) = headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) {
        return site == "same-origin";
    }
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        // 둘 다 없으면 거부합니다.
        return false;
    };
    let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    origin
        .parse::<axum::http::Uri>()
        .ok()
        .and_then(|u| u.authority().map(|a| a.as_str().to_string()))
        .map(|a| a == host)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(), HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn a_post_with_neither_header_is_rejected() {
        // 브라우저는 둘 중 하나를 반드시 붙입니다. 없으면 브라우저가 아닙니다.
        assert!(!same_origin(&headers(&[]), &Uri::from_static("/")));
    }

    #[test]
    fn sec_fetch_site_wins_when_present() {
        assert!(same_origin(&headers(&[("sec-fetch-site", "same-origin")]), &Uri::from_static("/")));
        // same-site·cross-site·none 은 전부 거부합니다.
        for v in ["same-site", "cross-site", "none"] {
            assert!(
                !same_origin(&headers(&[("sec-fetch-site", v)]), &Uri::from_static("/")),
                "sec-fetch-site={v} 를 통과시켰습니다"
            );
        }
    }

    #[test]
    fn sec_fetch_site_is_preferred_over_a_forged_origin() {
        // Origin 은 위조할 수 있지만 Sec-Fetch-Site 는 브라우저가 붙입니다.
        let h = headers(&[("sec-fetch-site", "cross-site"), ("origin", "http://console.local"), ("host", "console.local")]);
        assert!(!same_origin(&h, &Uri::from_static("/")));
    }

    #[test]
    fn origin_is_compared_against_host_when_sec_fetch_site_is_absent() {
        let same = headers(&[("origin", "http://console.local"), ("host", "console.local")]);
        assert!(same_origin(&same, &Uri::from_static("/")));

        let evil = headers(&[("origin", "http://evil.example"), ("host", "console.local")]);
        assert!(!same_origin(&evil, &Uri::from_static("/")));
    }

    #[test]
    fn a_console_without_a_resolver_cannot_be_built() {
        // 인증 없는 콘솔은 없는 것보다 나쁩니다.
        let ops = Arc::new(
            Service::new(crate::ops::Deps {
                audit: Arc::new(NoAudit),
                approvals: Arc::new(NoApprovals),
                workflows: None,
                inventory: None,
                registry: None,
                adapters: vec![],
                principals: vec![],
                tokens: None,
                principal_path: None,
                policy: None,
                policy_path: None,
                systems_path: None,
                catalog: None,
                reload_stamp_path: None,
                adapter_factory: None,
                systems_options: Default::default(),
                roles: vec![],
                budget: None,
                breakers: None,
                eval: None,
                recorder: None,
            })
            .unwrap(),
        );
        assert!(Console::new(ops, None, ADMIN_ROLE).is_err());
    }

    #[derive(Debug)]
    struct NoAudit;
    #[async_trait::async_trait]
    impl crate::audit::Reader for NoAudit {
        async fn query(&self, _f: &crate::audit::Filter) -> anyhow::Result<Vec<crate::audit::Entry>> { Ok(vec![]) }

        async fn recent(&self, _l: i64) -> anyhow::Result<Vec<crate::audit::Entry>> { Ok(vec![]) }

        async fn stats(&self, _b: crate::audit::GroupBy, _s: Option<chrono::DateTime<chrono::Utc>>) -> anyhow::Result<Vec<crate::audit::Stat>> { Ok(vec![]) }

        async fn oldest(&self) -> anyhow::Result<std::collections::HashMap<String, chrono::DateTime<chrono::Utc>>> { Ok(Default::default()) }
    }

    #[derive(Debug)]
    struct NoApprovals;
    #[async_trait::async_trait]
    impl crate::approval::Store for NoApprovals {
        async fn ensure(
            &self,
            _a: &str,
            _t: &str,
            _g: &serde_json::Map<String, serde_json::Value>,
            _ttl: std::time::Duration,
        ) -> Result<crate::approval::Request, crate::approval::Error> {
            Err(crate::approval::Error::NotFound)
        }

        async fn decide(&self, _i: &str, _a: bool, _b: &str, _n: &str) -> Result<crate::approval::Request, crate::approval::Error> {
            Err(crate::approval::Error::NotFound)
        }

        async fn get(&self, _i: &str) -> Result<crate::approval::Request, crate::approval::Error> { Err(crate::approval::Error::NotFound) }

        async fn list(&self, _s: Option<crate::approval::Status>, _l: i64) -> Result<Vec<crate::approval::Request>, crate::approval::Error> { Ok(vec![]) }
    }
}
