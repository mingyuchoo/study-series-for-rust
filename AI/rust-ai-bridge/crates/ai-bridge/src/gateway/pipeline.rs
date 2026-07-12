//! 파이프라인 — 사전 통제 · 실행 · 사후 처리.

use super::{Call,
            CallResult,
            Error,
            Gateway,
            execute::adapter_error,
            is_denial,
            new_request_id,
            obligations,
            output};
use crate::{audit,
            auth::Identity,
            policy::Decision,
            registry::{RiskLevel,
                       Sensitivity,
                       Spec,
                       Tool},
            schema,
            telemetry::CallInfo};
use anyhow::Result;
use chrono::{DateTime,
             Utc};
use serde_json::{Map,
                 Value};

/// 한 호출의 진행 상태.
struct Pipeline<'a> {
    g: &'a Gateway,
    id: Identity,
    call: Call,
    request_id: String,
    start: DateTime<Utc>,
    budget_key: String,

    tool: Option<Tool>,
    spec: Spec,
    decision: Decision,
    allow_reason: String,
    approval_status: String,
    approval_id: String,
    approval_ttl: std::time::Duration,
    /// 인젝션 신호 — 프롬프트와 출력 양쪽에서 모입니다.
    injection_flag: String,
}

impl Gateway {
    /// 도구 호출 하나를 처리합니다.
    pub async fn handle(&self, id: &Identity, call: Call) -> Result<CallResult, Error> {
        let request_id = new_request_id();
        let start = self.clock.now();

        let span = self.telemetry.as_ref().map(|t| t.start_call(&call.tool, &request_id, &id.session_id));

        let mut p = Pipeline {
            g: self,
            id: id.clone(),
            budget_key: if id.session_id.is_empty() {
                id.user_id.clone()
            } else {
                id.session_id.clone()
            },
            request_id: request_id.clone(),
            start,
            tool: None,
            spec: Spec::default(),
            decision: Decision::default(),
            allow_reason: String::new(),
            approval_status: "n/a".into(),
            approval_id: String::new(),
            approval_ttl: std::time::Duration::ZERO,
            injection_flag: String::new(),
            call,
        };

        // **프롬프트 인젝션은 결과와 무관하게 먼저 표시합니다.** 거부된 호출에 딸린
        // 시도야말로 남겨야 할 신호입니다.
        let verdict = self.injection.scan(&p.call.prompt);
        if verdict.suspicious {
            p.injection_flag = format!("프롬프트: {verdict}");
        }

        let result = p.run().await;

        // 성공·실패와 무관하게 계측합니다. 거부 경로에서 계측을 빼먹으면 대시보드가
        // "요청이 없었다"고 말합니다.
        if let (Some(t), Some(span)) = (self.telemetry.as_ref(), span) {
            let (decision, code) = match &result {
                | Ok(_) => ("allowed", "ok".to_string()),
                | Err(e) => {
                    let d = if is_denial(&e.code) { "denied" } else { "allowed" };
                    (d, e.code.clone())
                },
            };
            t.end_call(
                span,
                CallInfo {
                    tool: p.call.tool.clone(),
                    system: p.spec.system.clone(),
                    decision: decision.to_string(),
                    error_code: code,
                    latency: (self.clock.now() - start).to_std().unwrap_or_default(),
                    usage: p.call.usage,
                },
            );
        }

        result
    }
}

impl Pipeline<'_> {
    async fn run(&mut self) -> Result<CallResult, Error> {
        // --- 사전 통제 ---
        if let Some(early) = self.admit().await? {
            return Ok(early); // dry-run
        }

        // --- 어댑터 실행 ---
        let tool = self.tool.clone().expect("admit() guarantees a tool");
        let raw = match self.g.execute(&tool, &self.id, &self.call.args).await {
            | Ok(v) => v,
            | Err(err) => {
                let gw = adapter_error(&err, &self.spec);
                // **인가는 통과했고 시스템이 아픈 것이므로 `allowed` 로 감사합니다.**
                let _ = self
                    .record("allowed", &format!("어댑터 실행 오류({})", gw.code), None, false, &err.to_string())
                    .await;
                return Err(gw);
            },
        };

        // --- 사후 처리 ---
        self.shape_output(raw).await
    }

    /// 사전 통제. `Ok(Some(_))` 이면 dry-run 으로 조기 반환합니다.
    async fn admit(&mut self) -> Result<Option<CallResult>, Error> {
        let tool_name = self.call.tool.clone();

        // --- A. 도구 조회 (allowlist) ---
        // 등록되지 않은 도구는 존재하지 않습니다. 임의 SQL·API·쉘은 표현할 방법이
        // 없습니다.
        let Some(tool) = self.g.registry.lookup(&tool_name) else {
            let _ = self.record("denied", "등록되지 않은 도구", None, false, "unknown tool").await;
            return Err(Error::new("not_found", format!("도구 {tool_name:?} 은(는) 등록되어 있지 않습니다."), ""));
        };
        self.spec = tool.spec.clone();
        self.tool = Some(tool);

        // --- B. 레이트 리밋 ---
        let limit = self.g.rate_limit_for(self.spec.rate_limit_per_min);
        let key = format!("{}:{}", self.id.user_id, tool_name);
        if !self.g.limiter.allow(&key, limit).await {
            let _ = self.record("denied", "레이트 리밋 초과", None, false, "rate limited").await;
            return Err(Error::new(
                "rate_limited",
                format!("도구 {tool_name:?} 호출이 분당 {limit}회 한도를 넘었습니다."),
                &self.spec.fallback,
            ));
        }

        // --- C. 비용 상한 ---
        // 이미 쓴 비용은 되돌릴 수 없으므로, 상한은 "여기서 멈춘다"는 뜻입니다.
        if let Err(e) = self.g.budget.check(&self.budget_key).await {
            let _ = self.record("denied", "세션 비용 상한 초과", None, false, &e.to_string()).await;
            return Err(Error::new("budget_exceeded", e.to_string(), &self.spec.fallback));
        }

        // --- D. 입력 검증 (JSON Schema Draft 2020-12) ---
        let args_value = Value::Object(self.call.args.clone());
        if let Err(e) = schema::validate(&args_value, &self.spec.input_schema) {
            let _ = self.record("denied", "입력 스키마 검증 실패", None, false, &e.to_string()).await;
            return Err(Error::new(
                "invalid_input",
                format!("입력이 도구 계약을 만족하지 않습니다: {e}"),
                &self.spec.fallback,
            ));
        }

        // --- E. 정책 (RBAC → 주체 스코프 → ABAC) ---
        let decision = self.g.policy.evaluate(&self.id, &self.spec, &self.call.args);
        self.decision = decision.clone();
        if !decision.allowed {
            let _ = self
                .record("denied", &decision.reason, None, false, &format!("policy denied: {}", decision.rule_id))
                .await;
            return Err(Error::new(
                "permission_denied",
                &decision.reason,
                // 구체적인 요청 경로를 안내합니다.
                format!("권한이 필요하면 {}", self.g.access_request_path(&self.spec.system)),
            ));
        }
        self.allow_reason = if decision.matched_rules.is_empty() {
            decision.reason.clone()
        } else {
            format!("{} [규칙: {}]", decision.reason, decision.matched_rules.join(", "))
        };

        // --- F. 고위험(L4) 차단 ---
        // 정책이 허용해도 무관합니다. 이것은 게이트웨이 차원의 스위치입니다.
        if self.spec.risk_level >= RiskLevel::L4 && !self.g.allow_high_risk {
            let _ = self.record("denied", "고위험(L4) 작업은 기본 차단됨", None, false, "high risk blocked").await;
            return Err(Error::new(
                "high_risk_blocked",
                format!("{} 도구 {tool_name:?} 은(는) 보안 심사 전까지 실행할 수 없습니다.", self.spec.risk_level),
                &self.spec.fallback,
            ));
        }

        // --- G. 승인 관문 ---
        if decision.approval_required {
            let ap = match self.g.approver.approve(&self.id, &self.spec, &self.call.args).await {
                | Ok(a) => a,
                | Err(e) => {
                    // **fail-closed.** 승인 저장소에 장애가 나면 실행을 중단합니다.
                    let _ = self.record("denied", "승인 처리 오류", None, false, &e.to_string()).await;
                    return Err(Error::new(
                        "approval_error",
                        format!("승인 상태를 확인할 수 없어 실행을 중단했습니다: {e}"),
                        &self.spec.fallback,
                    ));
                },
            };
            self.approval_status = ap.status.clone();
            self.approval_id = ap.request_id.clone();
            self.approval_ttl = ap.ttl;

            if ap.status == "rejected" {
                let _ = self.record("denied", "관리자가 승인을 거부함", None, false, "").await;
                return Err(Error::new(
                    "approval_rejected",
                    format!("승인 요청 {} 이(가) 거부되었습니다.", self.approval_id),
                    &self.spec.fallback,
                ));
            }

            if !ap.approved {
                // **dry-run.** 오류가 아닙니다 — 실행하지 않았을 뿐입니다.
                let summary = self.dry_run_summary();
                let _ = self
                    .record("allowed", &format!("승인 대기(dry-run) — {}", self.allow_reason), None, false, "")
                    .await;
                return Ok(Some(CallResult {
                    tool: tool_name,
                    dry_run: true,
                    approval_status: self.approval_status.clone(),
                    approval_id: self.approval_id.clone(),
                    request_id: self.request_id.clone(),
                    summary,
                    ..Default::default()
                }));
            }
        }

        // --- H. L3+ 사전 감사 (fail-closed) ---
        // **감사 실패가 실행을 막는 유일한 지점입니다.** 되돌리기 어려운 행동을
        // 기록 없이 하지 않기 위함입니다.
        if self.spec.risk_level >= RiskLevel::L3 {
            let reason = format!("고위험 실행 의도(pre-execution) — {}", self.allow_reason);
            if self.record("allowed", &reason, None, false, "").await.is_err() {
                return Err(Error::new(
                    "audit_unavailable",
                    "감사 기록을 보장할 수 없어 실행을 중단했습니다.",
                    &self.spec.fallback,
                ));
            }
        }

        Ok(None)
    }

    /// 사후 처리 — 출력 검증 · 의무 · 마스킹 · 인젝션 · 감사.
    async fn shape_output(&mut self, raw: Value) -> Result<CallResult, Error> {
        // 구체 타입의 슬라이스·맵을 순수 JSON 으로 정규화합니다. 그러지 않으면 아래
        // 타입 분기가 조용히 건너뛰어 **정보가 새어나갑니다.**
        let raw = output::normalize(&raw);
        let raw_map = raw.as_object().cloned().unwrap_or_default();

        // --- J. 출력 검증 ---
        // 레거시 응답이 도구 계약을 지키는지 확인합니다.
        if !self.spec.output_schema.is_null()
            && let Err(e) = schema::validate(&raw, &self.spec.output_schema)
        {
            let _ = self.record("allowed", "출력 스키마 검증 실패", None, false, &e.to_string()).await;
            return Err(Error::new(
                "invalid_output",
                format!("레거시 응답이 도구 계약을 만족하지 않습니다: {e}"),
                &self.spec.fallback,
            ));
        }

        // --- K. 의무 집행 ---
        let obl = self.decision.obligations.clone();
        let (shaped, narrowed) = obligations::apply(&raw_map, &obl);

        // --- L. PII 마스킹 ---
        // 도구 명세의 마스킹 필드 ∪ 정책 의무의 마스킹 필드.
        let mut mask_fields = self.spec.mask_fields.clone();
        mask_fields.extend(obl.mask_fields.iter().cloned());

        let masked = self.g.masker.mask(&Value::Object(shaped), &mask_fields);
        let mut masked_map = masked.as_object().cloned().unwrap_or_default();
        let mut is_masked = self.spec.sensitivity >= Sensitivity::Confidential || !mask_fields.is_empty();

        // --- M. 간접 인젝션 탐지 ---
        // **마스킹 전 원본을 검사합니다** — 마스킹이 신호를 지워 놓치지 않도록.
        let verdict = self.g.injection.scan(&raw.to_string());
        if verdict.suspicious {
            if self.injection_flag.is_empty() {
                self.injection_flag = format!("출력: {verdict}");
            } else {
                self.injection_flag.push_str(&format!("; 출력: {verdict}"));
            }
            // 탐지는 **파이프라인을 막지 않습니다**(오류가 아님). 다만 레거시가 돌려준
            // 지시문을 LLM 에게 그대로 넘기지 않도록 출력을 격리합니다.
            masked_map = serde_json::json!({
                "quarantined": true,
                "reason": "레거시 출력에서 프롬프트 인젝션 의심 콘텐츠가 탐지되어 격리되었습니다.",
                "patterns": verdict.patterns,
                "request_id": self.request_id,
            })
            .as_object()
            .unwrap()
            .clone();
            is_masked = true;
        }

        // --- N. 감사 로그 ---
        let _ = self.record("allowed", &self.allow_reason.clone(), Some(&masked_map), is_masked, "").await;

        Ok(CallResult {
            tool: self.call.tool.clone(),
            data: masked_map,
            masked: is_masked,
            narrowed,
            dry_run: false,
            approval_status: self.approval_status.clone(),
            approval_id: self.approval_id.clone(),
            summary: String::new(),
            request_id: self.request_id.clone(),
        })
    }

    /// 승인 대기 안내. **인자는 마스킹해서 보여줍니다.**
    fn dry_run_summary(&self) -> String {
        let safe = self.g.masker.mask(&Value::Object(self.call.args.clone()), &self.spec.mask_fields);
        format!(
            "{} 도구 {:?} 은(는) 승인이 필요합니다.\n  \
             사유: {}\n  \
             승인 요청 ID: {}\n  \
             대상 시스템: {}\n  \
             입력: {}\n  \
             승인 유효 기간: {:?} (승인 시점부터)\n  \
             관리자 승인: auditctl approve {} -by <요청자가 아닌 사람>",
            self.spec.risk_level, self.call.tool, self.decision.reason, self.approval_id, self.spec.system, safe, self.approval_ttl, self.approval_id,
        )
    }

    /// 감사 기록 + 비용 누적. **모든 종료 분기가 이것을 정확히 한 번
    /// 부릅니다.**
    async fn record(&self, decision: &str, reason: &str, output: Option<&Map<String, Value>>, masked: bool, error: &str) -> Result<()> {
        // **거부된 호출이라도 LLM 턴의 비용은 이미 발생했습니다.**
        self.g.budget.add(&self.budget_key, self.call.usage.cost_micros).await;

        let entry = audit::Entry {
            timestamp: self.g.clock.now(),
            actor: self.id.user_id.clone(),
            tool: self.call.tool.clone(),
            system: self.spec.system.clone(),
            access: self.spec.access.to_string(),
            decision: decision.to_string(),
            reason: reason.to_string(),
            approval_status: self.approval_status.clone(),
            approval_id: self.approval_id.clone(),
            request_id: self.request_id.clone(),
            session_id: self.id.session_id.clone(),
            masked,
            input: Some(self.call.args.clone()),
            output: output.cloned(),
            latency_ms: (self.g.clock.now() - self.start).num_milliseconds(),
            input_tokens: self.call.usage.input_tokens,
            output_tokens: self.call.usage.output_tokens,
            cost_micros: self.call.usage.cost_micros,
            error: error.to_string(),
            prompt: self.call.prompt.clone(),
            injection: self.injection_flag.clone(),
            id: 0,
        };

        if let Err(e) = self.g.audit.log(&entry).await {
            tracing::error!("[{}] audit log failed: {e}", self.request_id);
            return Err(e);
        }
        Ok(())
    }
}
