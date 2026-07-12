//! REST 전송.
//!
//! **오류 번역이 이 전송의 핵심입니다.**
//!
//! | | 재시도 가치 있음 | 자원 없음 |
//! |---|---|---|
//! | REST | 5xx, 429, 네트워크 오류 | 404 |

use super::{Operation,
            Transport,
            not_found};
use ai_bridge::transient;
use anyhow::{Result,
             anyhow};
use serde_json::Value;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub struct RestTransport {
    base_url: String,
    client: reqwest::Client,
}

impl RestTransport {
    pub fn new(base_url: &str) -> Result<Self> {
        let u = reqwest::Url::parse(base_url).map_err(|_| anyhow!("invalid REST base_url {base_url:?}"))?;
        if u.host_str().is_none() {
            return Err(anyhow!("invalid REST base_url {base_url:?}"));
        }
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::builder().timeout(TIMEOUT).build()?,
        })
    }

    fn url(&self, op: &Operation) -> String {
        let path = op.path.iter().map(|s| urlencoding(s)).collect::<Vec<_>>().join("/");
        format!("{}/{}", self.base_url, path)
    }
}

#[async_trait::async_trait]
impl Transport for RestTransport {
    async fn call(&self, op: &Operation) -> Result<Value> {
        let url = self.url(op);
        let req = if op.write {
            self.client.post(&url).json(&op.params)
        } else {
            // 조회 조건은 질의 문자열로 나갑니다.
            let qs: Vec<String> = op
                .params
                .iter()
                .map(|(k, v)| {
                    let s = match v {
                        | Value::String(s) => s.clone(),
                        | other => other.to_string(),
                    };
                    format!("{}={}", urlencoding(k), urlencoding(&s))
                })
                .collect();
            let full = if qs.is_empty() { url.clone() } else { format!("{url}?{}", qs.join("&")) };
            self.client.get(&full)
        };

        // 네트워크 오류는 일시적 장애입니다.
        let resp = req.send().await.map_err(transient::temporary)?;
        let status = resp.status();

        // 404 는 "원래 없는 것" — 업무 오류이므로 재시도하지 않고 브레이커에도 먹이지
        // 않습니다.
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(not_found(format!("{} ({})", op.name, op.path.join("/"))));
        }
        // 5xx·429 는 "지금 안 되는 것" — 재시도할 가치가 있습니다.
        if status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(transient::temporary(anyhow!("legacy REST {} returned {status}", op.name)));
        }
        if !status.is_success() {
            return Err(anyhow!("legacy REST {} returned {status}", op.name));
        }

        resp.json::<Value>().await.map_err(|e| anyhow!("decode REST response for {}: {e}", op.name))
    }

    async fn health(&self) -> Result<()> {
        let resp = self
            .client
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .map_err(transient::temporary)?;
        if !resp.status().is_success() {
            return Err(transient::temporary(anyhow!("health returned {}", resp.status())));
        }
        Ok(())
    }

    fn describe(&self) -> String { format!("rest {}", self.base_url) }
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_ascii_alphanumeric() || "-_.~".contains(c) {
                vec![c.to_string()]
            } else {
                c.to_string().as_bytes().iter().map(|b| format!("%{b:02X}")).collect()
            }
        })
        .collect()
}
