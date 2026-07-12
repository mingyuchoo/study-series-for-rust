//! 콘솔 템플릿 (maud — **자동 이스케이프**).
//!
//! 감사 로그에는 LLM 이 만든 문자열과 사용자 프롬프트가 그대로 들어 있습니다.
//! 이스케이프 하지 않으면 프롬프트 인젝션이 관리자 브라우저까지 이어집니다.
//! `maud` 는 모든 보간을 자동으로 이스케이프하므로, 우리가 실수할 여지 자체가
//! 없습니다.
//!
//! **스크립트가 하나도 없습니다.** CSP 가 `default-src 'none'` 이라 애초에
//! 실행되지 않습니다.

use axum::{http::{HeaderValue,
                  StatusCode,
                  header},
           response::{IntoResponse,
                      Response}};
use chrono::{DateTime,
             Utc};
use maud::{DOCTYPE,
           Markup,
           PreEscaped,
           html};

/// 렌더링된 페이지 — 보안 헤더를 함께 붙입니다.
pub struct Page(pub Markup);

impl IntoResponse for Page {
    fn into_response(self) -> Response {
        let mut r = (StatusCode::OK, self.0.into_string()).into_response();
        let h = r.headers_mut();
        h.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
        // **스크립트 전면 금지.** 인라인 스타일만 허용합니다.
        h.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'"),
        );
        h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
        h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
        r
    }
}

const STYLE: &str = r#"
:root { --fg:#1a1a1a; --muted:#6b7280; --line:#e5e7eb; --bg:#fff;
        --deny:#b91c1c; --allow:#15803d; --warn:#b45309; --accent:#1d4ed8; }
* { box-sizing: border-box; }
body { margin:0; font:14px/1.6 -apple-system,BlinkMacSystemFont,"Segoe UI",
       "Noto Sans KR",sans-serif; color:var(--fg); background:var(--bg); }
header { border-bottom:1px solid var(--line); padding:12px 20px; display:flex;
         align-items:baseline; gap:16px; flex-wrap:wrap; }
header .brand { font-weight:700; }
header .viewer { margin-left:auto; color:var(--muted); font-size:12px; }
nav { display:flex; gap:14px; flex-wrap:wrap; }
nav a { color:var(--muted); text-decoration:none; }
nav a.on { color:var(--fg); font-weight:600; border-bottom:2px solid var(--accent); }
main { padding:20px; max-width:1400px; }
h1 { font-size:18px; margin:0 0 4px; }
p.lede { color:var(--muted); margin:0 0 16px; }
table { border-collapse:collapse; width:100%; font-size:13px; }
th,td { text-align:left; padding:7px 10px; border-bottom:1px solid var(--line);
        vertical-align:top; }
th { color:var(--muted); font-weight:600; white-space:nowrap; }
td.num { text-align:right; font-variant-numeric:tabular-nums; }
.cards { display:flex; gap:12px; flex-wrap:wrap; margin-bottom:20px; }
.card { border:1px solid var(--line); border-radius:8px; padding:12px 16px; min-width:120px; }
.card .n { font-size:22px; font-weight:700; }
.card .l { color:var(--muted); font-size:12px; }
.tag { display:inline-block; padding:1px 7px; border-radius:10px; font-size:12px; }
.tag.allow { background:#dcfce7; color:var(--allow); }
.tag.deny { background:#fee2e2; color:var(--deny); }
.tag.warn { background:#fef3c7; color:var(--warn); }
.tag.muted { background:#f3f4f6; color:var(--muted); }
.note { border:1px solid var(--line); border-left:3px solid var(--accent);
        padding:10px 14px; margin-bottom:16px; color:var(--muted); border-radius:4px; }
form.inline { display:inline; }
button { font:inherit; padding:3px 10px; border:1px solid var(--line);
         background:#fff; border-radius:5px; cursor:pointer; }
button.primary { border-color:var(--allow); color:var(--allow); }
button.danger { border-color:var(--deny); color:var(--deny); }
input,select,textarea { font:inherit; padding:4px 7px; border:1px solid var(--line);
                        border-radius:5px; }
textarea { width:100%; font-family:ui-monospace,monospace; font-size:12px; }
.filters { display:flex; gap:8px; flex-wrap:wrap; margin-bottom:14px; align-items:center; }
code { font-family:ui-monospace,monospace; font-size:12px; background:#f3f4f6;
       padding:1px 5px; border-radius:4px; }
pre { background:#f9fafb; border:1px solid var(--line); padding:10px;
      border-radius:6px; overflow-x:auto; font-size:12px; white-space:pre-wrap; }
.muted { color:var(--muted); }
"#;

const NAV: &[(&str, &str)] = &[
    ("/", "대시보드"),
    ("/calls", "도구 호출"),
    ("/injection", "인젝션 의심"),
    ("/stats", "사용량·비용"),
    ("/approvals", "승인"),
    ("/workflows", "업무 흐름"),
    ("/retention", "보존 기간"),
    ("/health", "레거시 상태"),
    ("/inventory", "인벤토리"),
    ("/tools", "설정"),
    ("/agents", "에이전트"),
];

/// 공통 레이아웃.
pub fn layout(title: &str, nav: &str, viewer: &str, body: Markup) -> Page {
    Page(html! {
        (DOCTYPE)
        html lang="ko" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " — AI Integration Gateway" }
                style { (PreEscaped(STYLE)) }
            }
            body {
                header {
                    span .brand { "AI Integration Gateway" }
                    nav {
                        @for (href, label) in NAV {
                            a href=(href) .on[*href == nav] { (label) }
                        }
                    }
                    span .viewer { (viewer) }
                }
                main {
                    (body)
                }
            }
        }
    })
}

// --- 표시 헬퍼 ---

pub fn time(t: DateTime<Utc>) -> String { t.with_timezone(&chrono::Local).format("%m-%d %H:%M:%S").to_string() }

pub fn time_opt(t: Option<DateTime<Utc>>) -> String { t.map(time).unwrap_or_else(|| "-".into()) }

pub fn dash(s: &str) -> String { if s.is_empty() { "-".into() } else { s.to_string() } }

pub fn pct(part: i64, total: i64) -> String {
    if total == 0 {
        return "0%".into();
    }
    format!("{:.1}%", (part as f64 / total as f64) * 100.0)
}

/// 천 단위 구분.
pub fn won(n: i64) -> String {
    let neg = n < 0;
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if neg { format!("-{out}") } else { out }
}

/// **룬 단위**로 자릅니다 — 바이트로 자르면 한글이 깨집니다.
pub fn truncate(s: &str, n: usize) -> String {
    let count = s.chars().count();
    if count <= n {
        return s.to_string();
    }
    let mut out: String = s.chars().take(n).collect();
    out.push('…');
    out
}

pub fn duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs == 0 {
        return "-".into();
    }
    if secs >= 3600 {
        format!("{}시간", secs / 3600)
    } else if secs >= 60 {
        format!("{}분", secs / 60)
    } else {
        format!("{secs}초")
    }
}

/// 판단 → 태그.
pub fn decision_tag(decision: &str) -> Markup {
    let class = match decision {
        | "allowed" => "tag allow",
        | "denied" => "tag deny",
        | _ => "tag muted",
    };
    html! { span class=(class) { (decision) } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_is_escaped() {
        // 프롬프트에 섞인 스크립트가 관리자 브라우저에서 실행되면 안 됩니다.
        let evil = "<script>alert('xss')</script>";
        let m = html! { td { (evil) } };
        let s = m.into_string();
        assert!(!s.contains("<script>"), "이스케이프되지 않았습니다: {s}");
        assert!(s.contains("&lt;script&gt;"));
    }

    #[test]
    fn the_page_forbids_scripts_via_csp() {
        let page = layout("t", "/", "manager-01", html! { p { "x" } });
        let resp = page.into_response();
        let csp = resp.headers().get(header::CONTENT_SECURITY_POLICY).unwrap().to_str().unwrap();
        assert!(csp.contains("default-src 'none'"));
    }

    #[test]
    fn truncate_is_rune_safe() {
        // 바이트로 자르면 UTF-8 경계 중간에서 깨집니다.
        assert_eq!(truncate("안녕하세요 반갑습니다", 5), "안녕하세요…");
        assert_eq!(truncate("짧음", 10), "짧음");
    }

    #[test]
    fn won_groups_thousands() {
        assert_eq!(won(1_234_567), "1,234,567");
        assert_eq!(won(-1000), "-1,000");
        assert_eq!(won(0), "0");
        assert_eq!(won(999), "999");
    }

    #[test]
    fn pct_handles_zero_total() {
        assert_eq!(pct(0, 0), "0%");
        assert_eq!(pct(1, 4), "25.0%");
    }
}
