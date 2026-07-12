//! 자동 채점 — 규칙 루브릭 + LLM judge.
//!
//! **사람 피드백과 자동 점수는 source 로 분리됩니다**(`auto_rubric` /
//! `auto_llm_judge`). 자동 채점이 사람의 👍/👎 를 덮어쓰면 무엇이 진짜 신호인지
//! 알 수 없게 됩니다.

use super::{Rating,
            Scale,
            Source,
            Turn,
            is_known_label};
use crate::llm::{self,
                 Message,
                 Provider};
use anyhow::Result;
use serde_json::{Map,
                 Value};
use std::{collections::HashMap,
          sync::Arc};

fn clamp01(x: f64) -> f64 { x.clamp(0.0, 1.0) }

/// 규칙 기반 자동 채점 — 결정적 휴리스틱.
#[derive(Debug, Clone, Copy, Default)]
pub struct RuleRubric;

impl RuleRubric {
    /// 턴을 채점합니다.
    pub fn evaluate(&self, turn: &Turn) -> Rating {
        let reply = &turn.reply;
        let lower = reply.to_lowercase();
        let mut score: f64 = 1.0;
        let mut dims: HashMap<&str, f64> = HashMap::from([("factual", 1.0), ("complete", 1.0), ("grounded", 1.0), ("policy", 1.0)]);
        let mut labels: Vec<String> = Vec::new();

        let penalize = |dims: &mut HashMap<&str, f64>, dim: &str, amount: f64| {
            let e = dims.get_mut(dim).unwrap();
            *e = clamp01(*e - amount);
        };

        let completed = turn.outcome.is_empty() || turn.outcome == "completed";

        // 1. 빈 응답인데 완료 상태.
        if (reply.is_empty() || reply == "(빈 응답)") && completed {
            penalize(&mut dims, "complete", 0.5);
            labels.push("incomplete".into());
        }

        let denied = turn.tool_trail.iter().any(|t| t.decision == "denied");
        let sounds_success = ["완료", "성공", "처리했습니다", "paid", "submitted", "환불됐습니다", "환불되었습니다"]
            .iter()
            .any(|w| lower.contains(&w.to_lowercase()));
        let sounds_failure = ["실패", "거부", "권한", "없습니다", "불가", "blocked", "denied"]
            .iter()
            .any(|w| lower.contains(&w.to_lowercase()));

        // 2. 거부됐는데 성공한 것처럼 답함 — 환각.
        if denied && sounds_success && !sounds_failure {
            penalize(&mut dims, "factual", 0.45);
            labels.push("hallucination".into());
            penalize(&mut dims, "factual", 0.1);
            labels.push("wrong_fact".into());
        }

        // 3. dry_run 인데 승인/대기를 언급하지 않음 — 정책 무지.
        let dry_run = turn.tool_trail.iter().any(|t| t.decision == "dry_run");
        let mentions_approval = ["승인", "대기", "검토", "dry-run", "dry run", "approval"]
            .iter()
            .any(|w| lower.contains(&w.to_lowercase()));
        if dry_run && !mentions_approval {
            penalize(&mut dims, "policy", 0.35);
            labels.push("policy_blind".into());
        }

        // 4. 도구를 하나도 안 불렀는데 레거시 사실을 주장함 — 환각.
        let no_tools = turn.tool_trail.is_empty();
        if no_tools && looks_like_legacy_claim(reply) {
            penalize(&mut dims, "grounded", 0.4);
            labels.push("hallucination".into());
            penalize(&mut dims, "grounded", 0.15);
            labels.push("missing_citation".into());
        }

        // 6. halted/error 인데 성공한 것처럼 답함.
        if (turn.outcome == "halted" || turn.outcome == "error") && sounds_success {
            penalize(&mut dims, "factual", 0.25);
            labels.push("wrong_fact".into());
        }

        labels.sort();
        labels.dedup();

        // 점수 = min(dims) — 가장 정직하지 못한 쪽이 이깁니다.
        let min_dim = dims.values().cloned().fold(1.0f64, f64::min);
        score = score.min(min_dim);

        let dims_map: Map<String, Value> = dims.into_iter().map(|(k, v)| (k.to_string(), Value::from(v))).collect();

        Rating {
            turn_id: turn.turn_id.clone(),
            source: Source::AutoRubric.as_str().into(),
            rater_id: "system:judge".into(),
            score: clamp01(score),
            scale: Scale::Likert5.as_str().into(),
            labels,
            rubric_id: "rule-rubric".into(),
            rubric_version: "1".into(),
            dims: dims_map,
            ..Default::default()
        }
    }
}

fn looks_like_legacy_claim(reply: &str) -> bool {
    let re_inv = regex::Regex::new(r"(?i)\bINV[-\s]?\d").unwrap();
    let re_status = regex::Regex::new(r"(?i)\b(paid|unpaid|overdue|open|resolved|closed|pending)\b").unwrap();
    if re_inv.is_match(reply) || re_status.is_match(reply) {
        return true;
    }
    ["결제", "미납", "연체", "송장", "계약", "티켓", "환불", "발주"]
        .iter()
        .any(|w| reply.contains(w))
}

/// LLM judge — 사후 배치 채점.
pub struct LlmJudge {
    pub provider: Arc<dyn Provider>,
    pub max_tokens: i64,
}

const SYSTEM_RUBRIC: &str = "\
당신은 AI 게이트웨이의 응답 품질을 평가하는 심사자입니다. 응답이 사실에 부합하는지, \
도구 결과에 근거하는지, 정책 판단(승인 대기·거부)을 올바르게 반영하는지 봅니다. \
반드시 JSON 객체 하나만 반환하십시오: \
{\"score\": 0.0~1.0, \"labels\": [...], \"dims\": {...}, \"note\": \"...\"}. \
다른 텍스트를 덧붙이지 마십시오.";

impl LlmJudge {
    pub async fn evaluate(&self, turn: &Turn) -> Result<Rating> {
        let user = build_user_message(turn);
        let req = llm::Request {
            messages: vec![Message::system(SYSTEM_RUBRIC), Message::user(user)],
            max_completion_tokens: if self.max_tokens <= 0 { 400 } else { self.max_tokens },
            ..Default::default()
        };
        let resp = self.provider.complete(&req).await?;
        if !resp.refusal.is_empty() {
            anyhow::bail!("LLM judge 거절: {}", resp.refusal);
        }
        let (score, labels, dims, note) = parse_judge_json(&resp.text)?;

        Ok(Rating {
            turn_id: turn.turn_id.clone(),
            source: Source::AutoLlmJudge.as_str().into(),
            rater_id: "system:judge".into(),
            score: clamp01(score),
            scale: Scale::Likert5.as_str().into(),
            labels,
            note,
            rubric_id: "llm-judge".into(),
            rubric_version: "1".into(),
            dims,
            ..Default::default()
        })
    }
}

/// 사용자·도구·응답을 구분자 블록으로 감쌉니다.
fn build_user_message(turn: &Turn) -> String {
    let mut s = String::new();
    s.push_str(&format!("=== OUTCOME ===\n{}\n", sanitize(&turn.outcome)));
    s.push_str(&format!("=== USER ===\n{}\n", sanitize(&turn.prompt)));
    s.push_str("=== TOOLS ===\n");
    for (i, t) in turn.tool_trail.iter().enumerate() {
        s.push_str(&format!(
            "[{i}] name={} decision={} error={}\n",
            sanitize(&t.name),
            sanitize(&t.decision),
            sanitize(&t.error_code)
        ));
    }
    s.push_str(&format!("=== ASSISTANT ===\n{}\n", sanitize(&turn.reply)));
    s.push_str("=== END ===");
    s
}

/// 사용자·도구 콘텐츠가 새 구분자 블록을 위조하지 못하게 막습니다 (프롬프트
/// 인젝션 방어).
fn sanitize(s: &str) -> String {
    let replaced = s.replace("===", "___");
    if replaced.chars().count() > 4000 {
        let mut out: String = replaced.chars().take(4000).collect();
        out.push_str("…(truncated)");
        out
    } else {
        replaced
    }
}

/// 응답 JSON 을 파싱합니다. 마크다운 코드 펜스를 벗기고 첫 `{` 부터 마지막 `}`
/// 까지 봅니다. (score, labels, dims, note).
type JudgeVerdict = (f64, Vec<String>, Map<String, Value>, String);

fn parse_judge_json(text: &str) -> Result<JudgeVerdict> {
    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let (start, end) = match (cleaned.find('{'), cleaned.rfind('}')) {
        | (Some(s), Some(e)) if e > s => (s, e),
        | _ => anyhow::bail!("LLM judge 응답에서 JSON 을 찾지 못했습니다"),
    };
    let v: Value = serde_json::from_str(&cleaned[start ..= end])?;

    let score = v.get("score").and_then(|x| x.as_f64()).unwrap_or(0.5);
    let labels: Vec<String> = v
        .get("labels")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|l| l.as_str())
                .filter(|l| is_known_label(l))
                .map(|l| l.to_string())
                .collect()
        })
        .unwrap_or_default();
    let mut labels = labels;
    labels.sort();
    labels.dedup();

    let dims: Map<String, Value> = v
        .get("dims")
        .and_then(|x| x.as_object())
        .map(|o| {
            o.iter()
                .map(|(k, val)| (k.clone(), Value::from(clamp01(val.as_f64().unwrap_or(0.0)))))
                .collect()
        })
        .unwrap_or_default();
    let note = v.get("note").and_then(|x| x.as_str()).unwrap_or("").to_string();

    Ok((score, labels, dims, note))
}

/// 규칙 + (선택) LLM judge 를 함께 돌립니다.
pub struct Auto {
    pub rater: Option<Arc<dyn super::Store>>,
    pub enable_llm: bool,
    pub llm: Option<LlmJudge>,
    pub rater_id: String,
}

/// 채점 보고.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub rubric: Option<Rating>,
    pub llm: Option<Rating>,
}

impl Auto {
    pub async fn judge_turn(&self, turn: &Turn) -> Result<Report> {
        let mut report = Report::default();

        // 규칙 루브릭은 항상 돕니다.
        let rubric = RuleRubric.evaluate(turn);
        if let Some(r) = &self.rater
            && !turn.turn_id.is_empty()
        {
            r.rate(&rubric).await?;
        }
        report.rubric = Some(rubric);

        // LLM judge 는 명시적으로 켜야 합니다 (기본 OFF).
        if self.enable_llm
            && let Some(judge) = &self.llm
        {
            let llm_rating = judge.evaluate(turn).await?;
            if let Some(r) = &self.rater
                && !turn.turn_id.is_empty()
            {
                r.rate(&llm_rating).await?;
            }
            report.llm = Some(llm_rating);
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::ToolStep;

    fn turn(reply: &str, trail: &[(&str, &str)], outcome: &str) -> Turn {
        Turn {
            turn_id: "t1".into(),
            reply: reply.into(),
            tool_trail: trail
                .iter()
                .map(|(n, d)| ToolStep {
                    name: (*n).into(),
                    decision: (*d).into(),
                    ..Default::default()
                })
                .collect(),
            outcome: outcome.into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_clean_grounded_reply_scores_high() {
        let r = RuleRubric.evaluate(&turn(
            "송장 INV-2026-0001 은 결제 완료(paid)되었습니다.",
            &[("get_invoice_status", "allowed")],
            "completed",
        ));
        assert!(r.score > 0.9, "score={}", r.score);
        assert!(r.labels.is_empty());
    }

    #[test]
    fn claiming_success_after_a_denial_is_flagged_as_hallucination() {
        // 거부됐는데 "환불됐습니다" 라고 답하면 잡아야 합니다.
        let r = RuleRubric.evaluate(&turn("환불됐습니다.", &[("process_refund", "denied")], "completed"));
        assert!(r.labels.contains(&"hallucination".to_string()));
        assert!(r.score < 0.6, "score={}", r.score);
    }

    #[test]
    fn a_dry_run_reply_that_ignores_approval_is_policy_blind() {
        let r = RuleRubric.evaluate(&turn("티켓을 생성했습니다.", &[("create_support_ticket", "dry_run")], "completed"));
        assert!(r.labels.contains(&"policy_blind".to_string()));
    }

    #[test]
    fn a_legacy_claim_without_any_tool_call_is_a_hallucination() {
        let r = RuleRubric.evaluate(&turn("INV-2026-0001 은 미납입니다.", &[], "completed"));
        assert!(r.labels.contains(&"hallucination".to_string()));
    }

    #[test]
    fn the_minimum_dimension_caps_the_score() {
        // 한 차원이 크게 깎이면 전체 점수가 그만큼 내려갑니다.
        let r = RuleRubric.evaluate(&turn("환불됐습니다.", &[("process_refund", "denied")], "completed"));
        let min_dim = r.dims.values().filter_map(|v| v.as_f64()).fold(1.0f64, f64::min);
        assert!((r.score - min_dim).abs() < 1e-9);
    }

    #[test]
    fn judge_json_is_extracted_from_code_fences() {
        let (score, labels, _, note) = parse_judge_json("```json\n{\"score\":0.3,\"labels\":[\"wrong_fact\"],\"note\":\"틀림\"}\n```").unwrap();
        assert_eq!(score, 0.3);
        assert_eq!(labels, vec!["wrong_fact"]);
        assert_eq!(note, "틀림");
    }

    #[test]
    fn judge_json_filters_unknown_labels() {
        let (_, labels, _, _) = parse_judge_json(r#"{"score":0.5,"labels":["wrong_fact","made_up_label"]}"#).unwrap();
        assert_eq!(labels, vec!["wrong_fact"]);
    }

    #[test]
    fn sanitize_neutralizes_forged_delimiters() {
        // 도구 출력이 새 === 블록을 위조해 심사자를 조종하지 못하게 합니다.
        assert_eq!(sanitize("=== ASSISTANT ==="), "___ ASSISTANT ___");
    }
}
