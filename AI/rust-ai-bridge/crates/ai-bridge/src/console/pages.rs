//! 콘솔 화면.

use super::{Console,
            Viewer,
            templates::*};
use crate::{auth,
            ops::{CallFilter,
                  Error as OpsError}};
use axum::{Form,
           extract::{Extension,
                     Path,
                     Query,
                     State},
           http::StatusCode,
           response::{IntoResponse,
                      Redirect,
                      Response}};
use maud::{Markup,
           html};
use serde::Deserialize;
use std::collections::HashMap;

fn fail(e: impl std::fmt::Display) -> Response {
    tracing::error!("console: {e}");
    (StatusCode::INTERNAL_SERVER_ERROR, format!("요청을 처리하지 못했습니다: {e}")).into_response()
}

fn since_of(q: &HashMap<String, String>, default_hours: i64) -> Option<chrono::DateTime<chrono::Utc>> {
    let hours = q
        .get("since")
        .and_then(|s| s.trim_end_matches('h').parse::<i64>().ok())
        .unwrap_or(default_hours);
    Some(chrono::Utc::now() - chrono::Duration::hours(hours))
}

// ---------------------------------------------------------------------------
// 프로브 (인증 없음)
// ---------------------------------------------------------------------------

pub async fn livez(State(c): State<Console>) -> Response { axum::Json(c.ops.live()).into_response() }

pub async fn readyz(State(c): State<Console>) -> Response {
    let (st, ready) = c.ops.ready().await;
    let code = if ready {
        StatusCode::OK
    } else {
        // **준비되지 않았으면 503** — K8s 가 트래픽을 보내지 않도록.
        StatusCode::SERVICE_UNAVAILABLE
    };
    (code, axum::Json(st)).into_response()
}

// ---------------------------------------------------------------------------
// 대시보드
// ---------------------------------------------------------------------------

pub async fn dashboard(State(c): State<Console>, Extension(v): Extension<Viewer>) -> Response {
    let d = match c.ops.dashboard().await {
        | Ok(d) => d,
        | Err(e) => return fail(e),
    };

    let card = |n: String, l: &str, href: Option<&str>| -> Markup {
        html! {
            div .card {
                div .n { @match href {
                    Some(h) => a href=(h) { (n) },
                    None => (n),
                } }
                div .l { (l) }
            }
        }
    };

    let body = html! {
        h1 { "24시간 현황" }
        p .lede { "호출·거부·실패·마스킹·인젝션 의심, 승인 대기, 장애 시스템, 보상 실패 흐름." }

        div .cards {
            (card(d.calls.to_string(), "호출", Some("/calls")))
            (card(d.denied.to_string(), "거부", Some("/calls?decision=denied")))
            (card(d.errors.to_string(), "오류", Some("/calls?errors=1")))
            (card(d.masked.to_string(), "마스킹", Some("/calls?masked=1")))
            (card(d.flagged.to_string(), "인젝션 의심", Some("/injection")))
            (card(d.pending.to_string(), "승인 대기", Some("/approvals")))
            (card(d.unhealthy.to_string(), "장애 시스템", Some("/health")))
            (card(d.failed_flows.to_string(), "보상 실패", Some("/workflows?status=failed")))
        }

        @if d.failed_flows > 0 {
            div .note {
                strong { "보상 실패 흐름이 있습니다." }
                " 돈이 나갔는데 되돌리지 못한 상태일 수 있습니다. "
                a href="/workflows?status=failed" { "확인하세요" } "."
            }
        }

        h1 { "고비용 세션" }
        @if d.costly.is_empty() {
            p .lede { "기록된 비용이 없습니다. 오케스트레이터가 " code { "_meta" } " 로 비용을 보내야 집계됩니다." }
        } @else {
            table {
                thead { tr { th { "세션" } th { "비용(micros)" } } }
                tbody {
                    @for e in &d.costly {
                        tr {
                            td { code { (e.key) } }
                            td .num { (won(e.spent_micros)) }
                        }
                    }
                }
            }
        }

        @if d.eval_enabled {
            h1 { "품질 평가" }
            div .cards {
                (card(d.eval_turns.to_string(), "턴", None))
                (card(d.eval_thumbs_rate.clone(), "👍 비율", None))
                (card(d.eval_unrated.to_string(), "미평가", None))
            }
        }
    };

    layout("대시보드", "/", &v.0.user_id, body).into_response()
}

// ---------------------------------------------------------------------------
// 도구 호출 이력
// ---------------------------------------------------------------------------

fn calls_table(rows: &[crate::ops::CallRow]) -> Markup {
    html! {
        table {
            thead {
                tr {
                    th { "시각" } th { "요청ID" } th { "주체" } th { "도구" }
                    th { "판단" } th { "승인" } th { "마스킹" } th { "지연" }
                    th { "비용" } th { "프롬프트" } th { "사유" }
                }
            }
            tbody {
                @for r in rows {
                    tr {
                        td { (time(r.timestamp)) }
                        td { code { (dash(&r.request_id)) } }
                        td { (r.actor) }
                        td { (r.tool) br; span .muted { (r.system) } }
                        td { (decision_tag(&r.decision)) }
                        td { (dash(&r.approval_status)) }
                        td { @if r.masked { "✓" } @else { "-" } }
                        td .num { (r.latency_ms) "ms" }
                        td .num { (won(r.cost_micros)) }
                        // maud 가 자동 이스케이프하므로 프롬프트 인젝션이 여기서 멈춥니다.
                        td { (truncate(&r.prompt, 40)) }
                        td {
                            (truncate(&r.reason, 60))
                            @if !r.error.is_empty() {
                                br; span .tag.deny { (truncate(&r.error, 60)) }
                            }
                        }
                    }
                }
            }
        }
        @if rows.is_empty() {
            p .lede { "기록이 없습니다." }
        }
    }
}

pub async fn calls(State(c): State<Console>, Extension(v): Extension<Viewer>, Query(q): Query<HashMap<String, String>>) -> Response {
    let f = CallFilter {
        tool: q.get("tool").cloned().unwrap_or_default(),
        actor: q.get("actor").cloned().unwrap_or_default(),
        session_id: q.get("session").cloned().unwrap_or_default(),
        decision: q.get("decision").cloned().unwrap_or_default(),
        errors_only: q.contains_key("errors"),
        masked_only: q.contains_key("masked"),
        injection_only: false,
        since: since_of(&q, 24),
        limit: 100,
    };
    let rows = match c.ops.query_calls(&f).await {
        | Ok(r) => r,
        | Err(e) => return fail(e),
    };

    let body = html! {
        h1 { "도구 호출 이력" }
        p .lede { "거부된 호출도 남습니다. 프롬프트 원문이 포함되므로 취급에 주의하십시오." }

        form .filters method="get" {
            input type="text" name="tool" placeholder="도구" value=(f.tool);
            input type="text" name="actor" placeholder="주체" value=(f.actor);
            input type="text" name="session" placeholder="세션" value=(f.session_id);
            select name="decision" {
                option value="" { "전체" }
                option value="allowed" selected[f.decision == "allowed"] { "허용" }
                option value="denied" selected[f.decision == "denied"] { "거부" }
            }
            label { input type="checkbox" name="errors" value="1" checked[f.errors_only]; " 오류만" }
            label { input type="checkbox" name="masked" value="1" checked[f.masked_only]; " 마스킹만" }
            button type="submit" { "조회" }
        }
        (calls_table(&rows))
    };

    layout("도구 호출", "/calls", &v.0.user_id, body).into_response()
}

// ---------------------------------------------------------------------------
// 인젝션 의심
// ---------------------------------------------------------------------------

pub async fn injection(State(c): State<Console>, Extension(v): Extension<Viewer>, Query(q): Query<HashMap<String, String>>) -> Response {
    let f = CallFilter {
        injection_only: true,
        since: since_of(&q, 24 * 7),
        limit: 100,
        ..Default::default()
    };
    let rows = match c.ops.query_calls(&f).await {
        | Ok(r) => r,
        | Err(e) => return fail(e),
    };

    let body = html! {
        h1 { "프롬프트 인젝션 의심 요청" }
        div .note {
            strong { "탐지는 차단이 아니라 표시입니다." }
            " 규칙 기반 휴리스틱은 새로운 우회를 놓치고, 정상 문서가 \"이전 지시를 무시하라\"를 \
              인용만 해도 걸립니다. 게이트웨이는 이 신호로 호출을 막지 않습니다 — 관리자가 \
              판단하십시오. 휴리스틱을 차단 근거로 쓰면 오탐이 곧 정상 업무의 거부가 됩니다."
        }
        table {
            thead {
                tr {
                    th { "시각" } th { "요청ID" } th { "주체" } th { "도구" }
                    th { "판단" } th { "걸린 규칙" } th { "프롬프트" }
                }
            }
            tbody {
                @for r in &rows {
                    tr {
                        td { (time(r.timestamp)) }
                        td { code { (dash(&r.request_id)) } }
                        td { (r.actor) }
                        td { (r.tool) }
                        td { (decision_tag(&r.decision)) }
                        td { span .tag.warn { (r.injection) } }
                        td { (truncate(&r.prompt, 80)) }
                    }
                }
            }
        }
        @if rows.is_empty() {
            p .lede { "인젝션 신호가 걸린 호출이 없습니다." }
        }
    };

    layout("인젝션 의심", "/injection", &v.0.user_id, body).into_response()
}

// ---------------------------------------------------------------------------
// 사용량 · 비용
// ---------------------------------------------------------------------------

pub async fn stats(State(c): State<Console>, Extension(v): Extension<Viewer>, Query(q): Query<HashMap<String, String>>) -> Response {
    let by = q.get("by").cloned().unwrap_or_else(|| "tool".into());
    let rows = match c.ops.call_stats(&by, since_of(&q, 24)).await {
        | Ok(r) => r,
        | Err(e) => return fail(e),
    };

    let body = html! {
        h1 { "사용량 · 비용" }
        p .lede { "사용자별 집계가 필요하면 여기를 보십시오 — 메트릭 레이블에는 넣지 않습니다." }

        form .filters method="get" {
            select name="by" {
                @for axis in ["tool", "actor", "system", "session"] {
                    option value=(axis) selected[by == axis] { (axis) }
                }
            }
            button type="submit" { "조회" }
        }

        table {
            thead {
                tr {
                    th { (by) } th { "호출" } th { "거부" } th { "오류" }
                    th { "거부율" } th { "평균지연" } th { "최대지연" } th { "비용" }
                }
            }
            tbody {
                @for s in &rows {
                    tr {
                        td { (s.key) }
                        td .num { (s.calls) }
                        td .num { (s.denied) }
                        td .num { (s.errors) }
                        td .num { (pct(s.denied, s.calls)) }
                        td .num { (format!("{:.1}ms", s.avg_latency_ms)) }
                        td .num { (s.max_latency_ms) "ms" }
                        td .num { (won(s.cost_micros)) }
                    }
                }
            }
        }
        @if rows.is_empty() { p .lede { "기록이 없습니다." } }
    };

    layout("사용량·비용", "/stats", &v.0.user_id, body).into_response()
}

// ---------------------------------------------------------------------------
// 승인
// ---------------------------------------------------------------------------

pub async fn approvals(State(c): State<Console>, Extension(v): Extension<Viewer>, Query(q): Query<HashMap<String, String>>) -> Response {
    let status = q.get("status").cloned().unwrap_or_else(|| "pending".into());
    let msg = q.get("msg").cloned().unwrap_or_default();
    let viewer = v.0.user_id.clone();

    let rows = match c.ops.list_approvals(&status, &viewer, 100).await {
        | Ok(r) => r,
        | Err(e) => return fail(e),
    };

    let body = html! {
        h1 { "승인" }
        div .note {
            strong { "결정자는 로그인한 관리자입니다." }
            " 폼으로 이름을 받지 않습니다 — 받으면 아무 이름이나 적어 넣을 수 있고 감사 추적이 \
              거짓이 됩니다. 그리고 "
            strong { "요청자는 자기 요청을 결정할 수 없습니다" }
            " — 이 검사는 승인 저장소가 하므로 콘솔이든 CLI 든 우회할 수 없습니다."
        }
        @if !msg.is_empty() { div .note { (msg) } }

        form .filters method="get" {
            select name="status" {
                @for s in ["pending", "approved", "rejected", "consumed", "expired", "all"] {
                    option value=(s) selected[status == s] { (s) }
                }
            }
            button type="submit" { "조회" }
        }

        table {
            thead {
                tr {
                    th { "요청ID" } th { "요청시각" } th { "주체" } th { "도구" }
                    th { "상태" } th { "결정자" } th { "남은시간" } th { "인자" } th { "" }
                }
            }
            tbody {
                @for r in &rows {
                    tr {
                        td { code { (r.id) } }
                        td { (time(r.requested_at)) }
                        td { (r.actor) }
                        td { (r.tool) }
                        td { (decision_tag(match r.status.as_str() {
                            "approved" => "allowed",
                            "rejected" | "expired" => "denied",
                            other => other,
                        })) " " span .muted { (r.status) } }
                        td { (dash(&r.decided_by)) }
                        td { (r.remaining) }
                        td { code { (truncate(&format!("{:?}", r.args), 40)) } }
                        td {
                            @if r.decidable {
                                form .inline method="post" action=(format!("/approvals/{}/decide", r.id)) {
                                    input type="hidden" name="action" value="approve";
                                    button .primary type="submit" { "승인" }
                                }
                                " "
                                form .inline method="post" action=(format!("/approvals/{}/decide", r.id)) {
                                    input type="hidden" name="action" value="reject";
                                    button .danger type="submit" { "거부" }
                                }
                            } @else if r.own {
                                span .muted { "본인 요청" }
                            }
                        }
                    }
                }
            }
        }
        @if rows.is_empty() { p .lede { "해당 상태의 승인 요청이 없습니다." } }
    };

    layout("승인", "/approvals", &viewer, body).into_response()
}

#[derive(Deserialize)]
pub struct DecideForm {
    action: String,
    #[serde(default)]
    note: String,
}

pub async fn decide(State(c): State<Console>, Extension(v): Extension<Viewer>, Path(id): Path<String>, Form(f): Form<DecideForm>) -> Response {
    let approve = match f.action.as_str() {
        | "approve" => true,
        | "reject" => false,
        | _ => return (StatusCode::BAD_REQUEST, "알 수 없는 요청입니다").into_response(),
    };

    // **결정자는 세션에서 옵니다** — 폼에 `by` 필드가 존재하지 않습니다.
    let me = v.0.user_id.clone();

    let msg = match c.ops.decide_approval(&id, approve, &me, &f.note).await {
        | Ok(()) => format!("요청 {id} → {} (결정자: {me})", if approve { "승인" } else { "거부" }),
        | Err(OpsError::SelfApproval(who)) => format!("요청자는 자기 요청을 결정할 수 없습니다. 다른 사람이 검토해야 합니다(요청자: {who})"),
        | Err(OpsError::NotPending) => format!("요청 {id} 은(는) 이미 결정되었습니다"),
        | Err(OpsError::NotFound) => format!("요청 {id} 을(를) 찾을 수 없습니다"),
        | Err(e) => return fail(e),
    };

    Redirect::to(&format!("/approvals?status=all&msg={}", urlencode(&msg))).into_response()
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 업무 흐름
// ---------------------------------------------------------------------------

pub async fn workflows(State(c): State<Console>, Extension(v): Extension<Viewer>, Query(q): Query<HashMap<String, String>>) -> Response {
    let status = q.get("status").cloned().unwrap_or_else(|| "all".into());
    let rows = match c.ops.list_workflows(&status, 100).await {
        | Ok(r) => r,
        | Err(e) => return fail(e),
    };

    let body = html! {
        h1 { "업무 흐름" }
        p .lede { "보상까지 실패한 흐름은 " strong { "사람이 손봐야 합니다" } " — 돈이 나갔는데 되돌리지 못한 상태일 수 있습니다." }

        form .filters method="get" {
            select name="status" {
                @for s in ["all", "running", "waiting", "completed", "compensated", "failed", "cancelled"] {
                    option value=(s) selected[status == s] { (s) }
                }
            }
            button type="submit" { "조회" }
        }

        table {
            thead {
                tr { th { "실행ID" } th { "흐름" } th { "상태" } th { "완료단계" } th { "갱신" } th { "사유" } }
            }
            tbody {
                @for r in &rows {
                    tr {
                        td { code { (r.id) } }
                        td { (r.name) }
                        td {
                            @match r.status.as_str() {
                                "completed" => span .tag.allow { (r.status) },
                                "failed" => span .tag.deny { (r.status) },
                                "compensated" => span .tag.warn { (r.status) },
                                _ => span .tag.muted { (r.status) },
                            }
                        }
                        td .num { (r.completed.len()) }
                        td { (time_opt(r.updated_at)) }
                        td {
                            @if !r.compensate_error.is_empty() {
                                strong .tag.deny { "보상 실패(사람 확인 필요)" }
                                br; (truncate(&r.compensate_error, 80))
                            } @else {
                                (truncate(&r.error, 80))
                            }
                        }
                    }
                }
            }
        }
        @if rows.is_empty() { p .lede { "해당 상태의 업무 흐름이 없습니다." } }
    };

    layout("업무 흐름", "/workflows", &v.0.user_id, body).into_response()
}

// ---------------------------------------------------------------------------
// 보존 기간
// ---------------------------------------------------------------------------

pub async fn retention(State(c): State<Console>, Extension(v): Extension<Viewer>) -> Response {
    let rows = match c.ops.retention().await {
        | Ok(r) => r,
        | Err(e) => return fail(e),
    };

    let body = html! {
        h1 { "보존 기간" }
        div .note {
            "삭제는 여기서 하지 않습니다. 감사 기록 삭제는 되돌릴 수 없으므로 "
            code { "auditctl retention -purge -archive-dir DIR" } " 로만 가능합니다."
        }
        table {
            thead {
                tr { th { "도구" } th { "보존(일)" } th { "가장 오래된 기록" } th { "상태" } }
            }
            tbody {
                @for r in &rows {
                    tr {
                        td { (r.tool) }
                        td .num { @if r.days == 0 { "영구" } @else { (r.days) } }
                        td { (time_opt(r.oldest)) }
                        td {
                            @if r.over { span .tag.deny { "보존 기간 초과" } }
                            @else { span .tag.allow { "정상" } }
                        }
                    }
                }
            }
        }
    };

    layout("보존 기간", "/retention", &v.0.user_id, body).into_response()
}

// ---------------------------------------------------------------------------
// 레거시 상태
// ---------------------------------------------------------------------------

pub async fn health(State(c): State<Console>, Extension(v): Extension<Viewer>) -> Response {
    let rows = c.ops.health().await;
    let breakers = c.ops.breakers();

    let body = html! {
        h1 { "레거시 상태" }
        p .lede { "장애 영향도와 담당 부서를 함께 보여줍니다 — 누구에게 연락할지 바로 알 수 있도록." }
        table {
            thead {
                tr { th { "시스템" } th { "상태" } th { "응답" } th { "장애영향도" } th { "담당부서" } th { "오류" } }
            }
            tbody {
                @for r in &rows {
                    tr {
                        td { (r.system) }
                        td {
                            @if r.healthy { span .tag.allow { "정상" } }
                            @else { span .tag.deny { "장애" } }
                        }
                        td .num { (r.latency.as_millis()) "ms" }
                        td { (dash(&r.impact)) }
                        td { (dash(&r.owner)) @if !r.contact.is_empty() { br; span .muted { (r.contact) } } }
                        td { (truncate(&r.error, 60)) }
                    }
                }
            }
        }

        h1 { "서킷 브레이커" }
        @if breakers.is_empty() {
            p .lede { "열린 회로가 없습니다." }
        } @else {
            table {
                thead { tr { th { "시스템" } th { "상태" } th { "실패" } th { "재시도까지" } } }
                tbody {
                    @for b in &breakers {
                        tr {
                            td { (b.system) }
                            td {
                                @if b.state == "closed" { span .tag.allow { (b.state) } }
                                @else { span .tag.deny { (b.state) } }
                            }
                            td .num { (b.failures) }
                            td { (duration(b.retry_in)) }
                        }
                    }
                }
            }
        }
    };

    layout("레거시 상태", "/health", &v.0.user_id, body).into_response()
}

// ---------------------------------------------------------------------------
// 인벤토리
// ---------------------------------------------------------------------------

pub async fn inventory(State(c): State<Console>, Extension(v): Extension<Viewer>) -> Response {
    let (systems, tools, counts) = c.ops.inventory();

    let body = html! {
        h1 { "레거시 시스템 인벤토리" }
        p .lede { "이 파일은 문서가 아니라 코드가 읽는 설정입니다. 인벤토리에 없는 시스템의 도구는 등록되지 않습니다." }
        table {
            thead {
                tr {
                    th { "시스템" } th { "이름" } th { "인터페이스" } th { "기능" }
                    th { "민감도" } th { "담당부서" } th { "영향도" } th { "도구 수" }
                }
            }
            tbody {
                @for s in &systems {
                    tr {
                        td { code { (s.name) } }
                        td { (s.display_name) }
                        td { (s.interface) }
                        td { (s.capabilities.join(" ")) }
                        td { (s.data_sensitivity.join(" ")) }
                        td { (s.owner_team) br; span .muted { (s.contact) } }
                        td { (s.failure_impact) }
                        td .num { (counts.get(&s.name).copied().unwrap_or(0)) }
                    }
                }
            }
        }

        h1 { "등록된 도구" }
        table {
            thead {
                tr {
                    th { "도구" } th { "시스템" } th { "접근" } th { "위험" }
                    th { "민감도" } th { "승인 유효기간" } th { "보존(일)" } th { "필요 권한" }
                }
            }
            tbody {
                @for t in &tools {
                    tr {
                        td { code { (t.name) } }
                        td { (t.system) }
                        td { (t.access) }
                        td { (t.risk_level) }
                        td { (t.sensitivity) }
                        td { (duration(t.approval_ttl)) }
                        td .num { (t.log_retention_days) }
                        td { @for p in &t.required_permissions { code { (p) } " " } }
                    }
                }
            }
        }
    };

    layout("인벤토리", "/inventory", &v.0.user_id, body).into_response()
}

// ---------------------------------------------------------------------------
// 설정 · 핫 리로드
// ---------------------------------------------------------------------------

fn tools_view(c: &Console, viewer: &str, message: &str, ok: bool) -> Response {
    let policy = c.ops.read_config_file(c.ops.policy_path());
    let systems = c.ops.read_config_file(c.ops.systems_path());
    let catalog = c.ops.read_config_file(c.ops.tools_catalog_path());

    let body = html! {
        h1 { "설정 · 핫 리로드" }
        div .note {
            "적용은 원자적입니다 — 임시 파일에 쓰고 rename 하며, 리로드가 실패하면 "
            strong { "이전 내용을 되돌립니다" }
            ". 반쯤 적용된 설정으로 게이트웨이가 도는 상태는 만들지 않습니다."
        }
        @if !message.is_empty() {
            div .note { @if ok { span .tag.allow { "성공" } } @else { span .tag.deny { "실패" } } " " (message) }
        }

        form method="post" {
            input type="hidden" name="intent" value="reload_bundle";
            button .primary type="submit" { "번들 리로드 (정책 → 인벤토리 → 카탈로그 → 주체)" }
        }
        " "
        form .inline method="post" {
            input type="hidden" name="intent" value="notify_cluster";
            button type="submit" { "클러스터에 알림" }
        }

        h1 { "정책 (policies.yaml)" }
        form method="post" {
            input type="hidden" name="intent" value="apply_policy";
            textarea name="body" rows="14" { (policy) }
            br; button type="submit" { "정책 적용" }
        }

        h1 { "인벤토리 (systems.yaml)" }
        form method="post" {
            input type="hidden" name="intent" value="apply_inventory";
            textarea name="body" rows="14" { (systems) }
            br; button type="submit" { "인벤토리 적용 (어댑터 재배선)" }
        }

        h1 { "동적 도구 카탈로그 (tools-dynamic.yaml)" }
        form method="post" {
            input type="hidden" name="intent" value="apply_catalog";
            textarea name="body" rows="10" { (catalog) }
            br; button type="submit" { "카탈로그 적용" }
        }
    };

    layout("설정", "/tools", viewer, body).into_response()
}

pub async fn tools(State(c): State<Console>, Extension(v): Extension<Viewer>) -> Response { tools_view(&c, &v.0.user_id, "", true) }

#[derive(Deserialize)]
pub struct ConfigForm {
    intent: String,
    #[serde(default)]
    body: String,
}

pub async fn tools_post(State(c): State<Console>, Extension(v): Extension<Viewer>, Form(f): Form<ConfigForm>) -> Response {
    let me = v.0.user_id.clone();

    let (msg, ok) = match f.intent.as_str() {
        | "apply_policy" => match c.ops.apply_policy_yaml(&me, &f.body).await {
            | Ok(()) => ("정책을 적용했습니다.".to_string(), true),
            | Err(e) => (format!("정책 적용 실패(되돌렸습니다): {e}"), false),
        },
        | "apply_inventory" => match c.ops.apply_inventory_yaml(&me, &f.body).await {
            | Ok(()) => ("인벤토리를 적용하고 어댑터를 재배선했습니다.".to_string(), true),
            | Err(e) => (format!("인벤토리 적용 실패(되돌렸습니다): {e}"), false),
        },
        | "apply_catalog" => match c.ops.apply_tools_catalog_yaml(&me, &f.body).await {
            | Ok((a, r)) => (format!("도구 카탈로그 적용: 추가 {a}, 제거 {r}"), true),
            | Err(e) => (format!("카탈로그 적용 실패(되돌렸습니다): {e}"), false),
        },
        | "reload_bundle" => {
            let res = c.ops.reload_config_bundle(&me).await;
            let summary: Vec<String> = res.steps.iter().map(|s| format!("{}={}", s.name, if s.ok { "OK" } else { "실패" })).collect();
            let msg = if res.rolled_back {
                format!("번들 리로드 실패 — 되돌렸습니다. [{}]", summary.join(", "))
            } else {
                format!("번들 리로드 완료. [{}]", summary.join(", "))
            };
            (msg, !res.rolled_back)
        },
        | "notify_cluster" => match c.ops.notify_cluster_reload(&me).await {
            | Ok(()) => ("클러스터에 리로드를 알렸습니다.".to_string(), true),
            | Err(e) => (e.to_string(), false),
        },
        | _ => ("알 수 없는 요청입니다".to_string(), false),
    };

    tools_view(&c, &me, &msg, ok)
}

// ---------------------------------------------------------------------------
// 에이전트 등록
// ---------------------------------------------------------------------------

fn agents_view(c: &Console, viewer: &str, message: &str, ok: bool, issued: Option<(String, String, String)>) -> Response {
    let cat = c.ops.agent_catalog();

    let body = html! {
        h1 { "에이전트 등록" }
        div .note {
            strong { "평문 토큰은 발급 직후 한 번만 보입니다." }
            " 파일에는 " code { "token_sha256" } " 만 저장됩니다. 에이전트는 역할이 준 권한 안에서 "
            code { "allowed_tools" } " / " code { "allowed_systems" } " 로 스코프를 "
            strong { "더 좁힐 수만" } " 있습니다 — 넓히지는 못합니다."
        }
        @if !message.is_empty() {
            div .note { @if ok { span .tag.allow { "성공" } } @else { span .tag.deny { "실패" } } " " (message) }
        }
        @if let Some((user, token, snippet)) = &issued {
            div .note {
                strong { "새 토큰이 발급되었습니다 — 다시 볼 수 없습니다." } br;
                "에이전트 " code { (user) } " 에만 안전하게 전달하십시오." br; br;
                code { "AGENT_TOKEN=" (token) }
                pre { (snippet) }
            }
        }

        h1 { "등록된 에이전트" }
        table {
            thead {
                tr { th { "user_id" } th { "역할" } th { "허용 도구" } th { "허용 시스템" } th { "토큰" } th { "만료" } th { "" } }
            }
            tbody {
                @for a in &cat.agents {
                    tr {
                        td { code { (a.user_id) } }
                        td { (a.roles.join(" ")) }
                        td { @if a.allowed_tools.is_empty() { span .muted { "제한 없음" } } @else { (a.allowed_tools.join(" ")) } }
                        td { @if a.allowed_systems.is_empty() { span .muted { "제한 없음" } } @else { (a.allowed_systems.join(" ")) } }
                        td { @if a.has_token { span .tag.allow { "있음" } } @else { span .tag.deny { "없음" } } }
                        td { (a.expiry) }
                        td {
                            form .inline method="post" action="/agents/register" {
                                input type="hidden" name="intent" value="remove";
                                input type="hidden" name="user_id" value=(a.user_id);
                                button .danger type="submit" { "삭제" }
                            }
                        }
                    }
                }
            }
        }
        @if cat.agents.is_empty() { p .lede { "등록된 에이전트가 없습니다." } }

        h1 { "새 에이전트 / 토큰 회전" }
        form method="post" action="/agents/register" {
            input type="hidden" name="intent" value="create";
            div .filters {
                input type="text" name="user_id" placeholder="user_id (예: agent-support-bot)" required;
                button .primary type="submit" { "등록 · 토큰 발급" }
            }
            p .lede { "역할" }
            div .filters {
                @for r in &cat.roles {
                    label { input type="checkbox" name="roles" value=(r); " " (r) }
                }
            }
            p .lede { "허용 도구 (비우면 역할 권한 전체)" }
            div .filters {
                @for t in &cat.tools {
                    label { input type="checkbox" name="tools" value=(t); " " (t) }
                }
            }
            p .lede { "허용 시스템 (비우면 제한 없음)" }
            div .filters {
                @for s in &cat.systems {
                    label { input type="checkbox" name="systems" value=(s); " " (s) }
                }
            }
        }
    };

    layout("에이전트", "/agents", viewer, body).into_response()
}

pub async fn agents(State(c): State<Console>, Extension(v): Extension<Viewer>) -> Response { agents_view(&c, &v.0.user_id, "", true, None) }

#[derive(Deserialize)]
pub struct AgentForm {
    intent: String,
    user_id: String,
    #[serde(default)]
    roles: Vec<String>,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    systems: Vec<String>,
}

pub async fn register_agent(State(c): State<Console>, Extension(v): Extension<Viewer>, Form(f): Form<AgentForm>) -> Response {
    let me = v.0.user_id.clone();

    if f.intent == "remove" {
        return match c.ops.remove_agent(&me, &f.user_id).await {
            | Ok(()) => agents_view(&c, &me, &format!("{} 을(를) 삭제했습니다.", f.user_id), true, None),
            | Err(e) => agents_view(&c, &me, &e.to_string(), false, None),
        };
    }

    // user_id 형식 검사.
    let valid = !f.user_id.is_empty() && f.user_id.len() <= 64 && f.user_id.chars().all(|ch| ch.is_ascii_alphanumeric() || "._-".contains(ch));
    if !valid {
        return agents_view(&c, &me, "user_id 형식이 올바르지 않습니다.", false, None);
    }
    if f.roles.is_empty() {
        return agents_view(&c, &me, "역할을 하나 이상 선택하세요.", false, None);
    }

    // **평문 토큰은 여기서 한 번만 존재합니다.** 파일에는 해시만 갑니다.
    let token = auth::generate_token();
    let hash = auth::token_hash(&token);

    let id = auth::Identity {
        user_id: f.user_id.clone(),
        kind: auth::KIND_AGENT.into(),
        roles: f.roles.clone(),
        allowed_tools: f.tools.clone(),
        allowed_systems: f.systems.clone(),
        token_sha256: hash.clone(),
        ..Default::default()
    };

    let action = if c.ops.principal_exists(&f.user_id) { "rotate" } else { "create" };

    match c.ops.apply_agent(&me, &id, action).await {
        | Ok(()) => {
            let snippet = agent_snippet(&f.user_id, &f.roles, &f.tools, &f.systems, &hash);
            agents_view(
                &c,
                &me,
                &format!("{} 을(를) {} 했습니다.", f.user_id, if action == "rotate" { "토큰 회전" } else { "등록" }),
                true,
                Some((f.user_id, token, snippet)),
            )
        },
        | Err(e) => agents_view(&c, &me, &format!("Apply 실패: {e}"), false, None),
    }
}

/// `principal.yaml` 에 붙일 YAML 조각 (표시 전용 — 적용은 이미 끝났습니다).
fn agent_snippet(user_id: &str, roles: &[String], tools: &[String], systems: &[String], hash: &str) -> String {
    let mut s = format!("  - user_id: {user_id}\n    kind: agent\n    roles:\n");
    for r in roles {
        s.push_str(&format!("      - {r}\n"));
    }
    if !tools.is_empty() {
        s.push_str("    allowed_tools:\n");
        for t in tools {
            s.push_str(&format!("      - {t}\n"));
        }
    }
    if !systems.is_empty() {
        s.push_str("    allowed_systems:\n");
        for x in systems {
            s.push_str(&format!("      - {x}\n"));
        }
    }
    s.push_str(&format!("    token_sha256: {hash}\n"));
    s
}
