//! SOAP 전송.
//!
//! **SOAP는 장애도 업무 오류도 HTTP 500 + Fault로 돌려줍니다.** 그래서 상태
//! 코드가 아니라 `faultcode` 를 봐야 합니다 — REST 와 결정적으로 다른
//! 지점입니다.
//!
//! | | 재시도 가치 있음 | 자원 없음 |
//! |---|---|---|
//! | SOAP | `soap:Server` fault | `soap:Client` fault + "not found" |
//!
//! ERP 어댑터는 **같은 코드**로 REST 와 SOAP 을 씁니다. 오류 번역만 여기서
//! 달라집니다.

use super::{Operation,
            Transport,
            not_found};
use ai_bridge::transient;
use anyhow::{Result,
             anyhow};
use serde_json::{Map,
                 Value};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub struct SoapTransport {
    endpoint: String,
    namespace: String,
    client: reqwest::Client,
}

impl SoapTransport {
    pub fn new(endpoint: &str) -> Result<Self> {
        let u = reqwest::Url::parse(endpoint).map_err(|_| anyhow!("invalid SOAP endpoint {endpoint:?}"))?;
        if u.host_str().is_none() {
            return Err(anyhow!("invalid SOAP endpoint {endpoint:?}"));
        }
        Ok(Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            namespace: "urn:ai-bridge:legacy".to_string(),
            client: reqwest::Client::builder().timeout(TIMEOUT).build()?,
        })
    }

    /// 업무 의도를 SOAP action 이름으로 바꿉니다: `get_invoice` → `GetInvoice`.
    fn action(&self, name: &str) -> String {
        name.split('_')
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    | Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    | None => String::new(),
                }
            })
            .collect()
    }

    fn envelope(&self, op: &Operation) -> String {
        let action = self.action(&op.name);
        let mut body = String::new();
        // 경로 조각도 파라미터로 실어 보냅니다.
        for (i, seg) in op.path.iter().enumerate() {
            body.push_str(&format!("<p{i}>{}</p{i}>", xml_escape(seg)));
        }
        for (k, v) in &op.params {
            let s = match v {
                | Value::String(s) => s.clone(),
                | other => other.to_string(),
            };
            body.push_str(&format!("<{k}>{}</{k}>", xml_escape(&s)));
        }
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Body>
    <{action} xmlns="{ns}">{body}</{action}>
  </soap:Body>
</soap:Envelope>"#,
            action = action,
            ns = self.namespace,
            body = body
        )
    }
}

#[async_trait::async_trait]
impl Transport for SoapTransport {
    async fn call(&self, op: &Operation) -> Result<Value> {
        let action = self.action(&op.name);
        let resp = self
            .client
            .post(&self.endpoint)
            .header("Content-Type", "text/xml; charset=utf-8")
            .header("SOAPAction", format!("\"{}/{}\"", self.namespace, action))
            .body(self.envelope(op))
            .send()
            .await
            .map_err(transient::temporary)?;

        let body = resp.text().await.map_err(|e| anyhow!("read SOAP response: {e}"))?;

        // **상태 코드가 아니라 faultcode 를 봅니다.**
        if let Some(fault) = parse_fault(&body) {
            let code = fault.0.to_lowercase();
            let reason = fault.1;
            // soap:Client + "not found" = 원래 없는 것.
            if code.contains("client") {
                if reason.to_lowercase().contains("not found") {
                    return Err(not_found(format!("{} ({})", op.name, op.path.join("/"))));
                }
                // 그 밖의 Client fault 는 업무 오류입니다 — 재시도해도 같습니다.
                return Err(anyhow!("SOAP fault [{}]: {reason}", fault.0));
            }
            // soap:Server = 지금 안 되는 것.
            return Err(transient::temporary(anyhow!("SOAP fault [{}]: {reason}", fault.0)));
        }

        if !resp_ok(&body) {
            return Err(anyhow!("SOAP response for {} was not understood", op.name));
        }
        Ok(parse_body(&body))
    }

    async fn health(&self) -> Result<()> {
        let resp = self.client.get(&self.endpoint).send().await.map_err(transient::temporary)?;
        // WSDL 조회는 보통 200 입니다. 5xx 만 장애로 봅니다.
        if resp.status().is_server_error() {
            return Err(transient::temporary(anyhow!("health returned {}", resp.status())));
        }
        Ok(())
    }

    fn describe(&self) -> String { format!("soap {}", self.endpoint) }
}

/// `<faultcode>`, `<faultstring>` 을 꺼냅니다.
fn parse_fault(xml: &str) -> Option<(String, String)> {
    if !xml.contains("Fault") {
        return None;
    }
    let code = tag_text(xml, "faultcode").unwrap_or_else(|| "soap:Server".into());
    let reason = tag_text(xml, "faultstring")
        .or_else(|| tag_text(xml, "Reason"))
        .unwrap_or_else(|| "unknown fault".into());
    Some((code, reason))
}

fn resp_ok(xml: &str) -> bool { xml.contains("Body") }

/// Body 안의 잎사귀 엘리먼트를 평평한 맵으로 바꿉니다.
///
/// 완전한 XML 파서가 아닙니다 — 참조 구현이 다루는 단순한 응답 형태를 위한
/// 것입니다.
fn parse_body(xml: &str) -> Value {
    let mut out = Map::new();
    let body = match (xml.find("Body>"), xml.rfind("</")) {
        | (Some(s), Some(_)) => &xml[s + 5 ..],
        | _ => xml,
    };
    let mut rest = body;
    while let Some(open) = rest.find('<') {
        let Some(close) = rest[open ..].find('>') else {
            break;
        };
        let tag_full = &rest[open + 1 .. open + close];
        if tag_full.starts_with('/') || tag_full.starts_with('?') {
            rest = &rest[open + close + 1 ..];
            continue;
        }
        let tag = tag_full.split_whitespace().next().unwrap_or("");
        let tag = tag.split(':').next_back().unwrap_or(tag);
        let after = &rest[open + close + 1 ..];
        let end_marker = format!("</{tag_full}>");
        let end_short = format!("</{tag}>");
        let end = after.find(&end_marker).or_else(|| after.find(&end_short));
        match end {
            | Some(e) => {
                let inner = &after[.. e];
                // 중첩이 없으면 잎사귀입니다.
                if !inner.contains('<') && !inner.trim().is_empty() {
                    out.insert(tag.to_string(), Value::String(xml_unescape(inner.trim())));
                }
                rest = after;
            },
            | None => {
                rest = after;
            },
        }
    }
    Value::Object(out)
}

fn tag_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let s = xml.find(&open)? + open.len();
    let e = xml[s ..].find(&close)? + s;
    Some(xml_unescape(xml[s .. e].trim()))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> SoapTransport { SoapTransport::new("https://erp.example/soap").unwrap() }

    #[test]
    fn intent_becomes_a_soap_action() {
        assert_eq!(t().action("get_invoice"), "GetInvoice");
        assert_eq!(t().action("list_customer_invoices"), "ListCustomerInvoices");
    }

    #[test]
    fn server_fault_is_transient_but_client_fault_is_not() {
        // 이 구분이 SOAP 전송의 존재 이유입니다.
        let server = r#"<soap:Fault><faultcode>soap:Server</faultcode>
            <faultstring>backend down</faultstring></soap:Fault>"#;
        let (code, reason) = parse_fault(server).unwrap();
        assert!(code.to_lowercase().contains("server"));
        assert_eq!(reason, "backend down");

        let client = r#"<soap:Fault><faultcode>soap:Client</faultcode>
            <faultstring>Invoice not found</faultstring></soap:Fault>"#;
        let (code, reason) = parse_fault(client).unwrap();
        assert!(code.to_lowercase().contains("client"));
        assert!(reason.to_lowercase().contains("not found"));
    }

    #[test]
    fn no_fault_when_response_is_clean() {
        assert!(parse_fault("<soap:Body><GetInvoiceResponse/></soap:Body>").is_none());
    }

    #[test]
    fn parses_leaf_elements_from_the_body() {
        let xml = r#"<soap:Envelope><soap:Body><GetInvoiceResponse>
            <invoice_id>INV-2026-0001</invoice_id>
            <status>paid</status>
            <amount>1200000</amount>
        </GetInvoiceResponse></soap:Body></soap:Envelope>"#;
        let v = parse_body(xml);
        assert_eq!(v["invoice_id"], serde_json::json!("INV-2026-0001"));
        assert_eq!(v["status"], serde_json::json!("paid"));
    }

    #[test]
    fn escapes_xml_special_characters() {
        let op = Operation::read("get_invoice").param("q", Value::String("a<b&c".into()));
        let env = t().envelope(&op);
        assert!(env.contains("a&lt;b&amp;c"));
        assert!(!env.contains("a<b&c"));
    }
}
