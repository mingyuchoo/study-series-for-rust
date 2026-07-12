//! K8s 프로브 — `/livez` · `/readyz` · `/healthz`.
//!
//! **인증이 없습니다.** 그래서 콘솔의 인증 미들웨어보다 **먼저** 매칭되어야
//! 합니다. 프로브는 상태 코드와 체크 목록만 내보내며, 프롬프트·감사 내용은 절대
//! 담지 않습니다.

use super::Service;
use serde::Serialize;
use std::collections::BTreeMap;

/// 프로브 응답.
#[derive(Debug, Clone, Serialize)]
pub struct ProbeStatus {
    /// `ok` | `not_ready`.
    pub status: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub checks: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unhealthy: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub version: String,
}

impl Service {
    /// 프로세스가 살아 있는가. **의존성을 보지 않습니다** — 살아 있으면 살아
    /// 있는 것입니다.
    pub fn live(&self) -> ProbeStatus {
        ProbeStatus {
            status: "ok".into(),
            checks: BTreeMap::new(),
            unhealthy: Vec::new(),
            version: format!("ai-bridge {}", env!("CARGO_PKG_VERSION")),
        }
    }

    /// 트래픽을 받을 준비가 되었는가.
    ///
    /// **어댑터 헬스까지 봅니다** — 레거시에 닿지 못하는 인스턴스에 트래픽을
    /// 보내면 사용자가 오류를 봅니다. 준비되지 않았으면 503 입니다.
    pub async fn ready(&self) -> (ProbeStatus, bool) {
        let mut st = ProbeStatus {
            status: "ok".into(),
            checks: BTreeMap::new(),
            unhealthy: Vec::new(),
            version: String::new(),
        };
        st.checks.insert("process".into(), "ok".into());

        let mark = |k: &str, present: bool, st: &mut ProbeStatus| {
            st.checks.insert(k.to_string(), if present { "ok".into() } else { "missing".to_string() });
            if !present {
                st.status = "not_ready".into();
            }
        };
        mark("tools", self.d.registry.is_some(), &mut st);
        mark("policy", self.d.policy.is_some(), &mut st);
        // audit 은 Deps 에서 필수이므로 언제나 존재합니다.
        st.checks.insert("audit".into(), "ok".into());

        for h in self.health().await {
            let key = format!("adapter:{}", h.system);
            if h.healthy {
                st.checks.insert(key, "ok".into());
            } else {
                let msg = if h.error.is_empty() { "unhealthy".to_string() } else { h.error.clone() };
                st.checks.insert(key, msg);
                st.unhealthy.push(h.system);
                st.status = "not_ready".into();
            }
        }

        let ready = st.status == "ok";
        (st, ready)
    }
}
