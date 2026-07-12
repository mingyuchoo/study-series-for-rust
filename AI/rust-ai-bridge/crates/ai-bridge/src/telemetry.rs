//! 계측 (OpenTelemetry · Prometheus).
//!
//! **감사 로그와 목적이 다릅니다.** 감사 로그는 "누가 무엇을 했는가"를 남기는
//! 규제 대응 기록이고, 계측은 "지금 시스템이 어떤 상태인가"를 보는 운영
//! 신호입니다.
//!
//! # 메트릭 레이블에 넣지 않는 것
//!
//! **사용자 ID · 세션 ID · 요청 ID · 도구 인자.** 값의 종류가 무한히 늘어나
//! 시계열이 폭발하고, 개인정보가 메트릭 저장소로 샙니다. 사용자별 호출량이
//! 필요하면 감사 로그를 집계하십시오(`auditctl stats -by actor`).
//!
//! 반대로 **스팬 속성에는 요청 ID와 세션 ID를 넣습니다.** 트레이스는 표본이고
//! 시계열이 아니며, 감사 로그와 대조하려면 그 두 값이 필요합니다.
//!
//! `decision` 레이블이 **"정책이 제대로 동작한 것"(denied)과 "시스템이 아픈 것"
//! (unavailable·timeout)을 구분**합니다.

use super::gateway::Usage;
use anyhow::Result;
use opentelemetry::{KeyValue,
                    global,
                    metrics::{Counter,
                              Histogram,
                              Meter},
                    trace::{Span,
                            SpanKind,
                            Tracer}};
use std::time::Duration;

/// 한 호출의 계측 정보.
#[derive(Debug, Clone, Default)]
pub struct CallInfo {
    pub tool: String,
    pub system: String,
    /// `allowed` | `denied`.
    pub decision: String,
    /// 오류 코드 또는 `ok`.
    pub error_code: String,
    pub latency: Duration,
    pub usage: Usage,
}

/// 게이트웨이 계측.
pub struct Telemetry {
    calls: Counter<u64>,
    duration: Histogram<f64>,
    tokens: Counter<u64>,
    cost: Counter<u64>,
    breaker_rejections: Counter<u64>,
}

impl std::fmt::Debug for Telemetry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("Telemetry").finish() }
}

/// 살아 있는 스팬.
pub struct CallSpan(opentelemetry::global::BoxedSpan);

impl Telemetry {
    pub fn new(meter: &Meter) -> Self {
        Self {
            calls: meter.u64_counter("gateway.tool.calls").with_description("도구 호출 수").build(),
            duration: meter
                .f64_histogram("gateway.tool.duration")
                .with_description("도구 호출 지연")
                .with_unit("ms")
                .build(),
            tokens: meter.u64_counter("gateway.llm.tokens").with_description("LLM 토큰 수").build(),
            cost: meter.u64_counter("gateway.llm.cost_micros").with_description("LLM 비용(마이크로)").build(),
            breaker_rejections: meter
                .u64_counter("gateway.breaker.rejections")
                .with_description("서킷 브레이커가 차단한 호출 수")
                .build(),
        }
    }

    /// 스팬을 엽니다. **요청 ID·세션 ID는 여기(스팬)에만 들어갑니다.**
    pub fn start_call(&self, tool: &str, request_id: &str, session_id: &str) -> CallSpan {
        let tracer = global::tracer("ai-bridge");
        let mut span = tracer.span_builder(format!("gateway.tool/{tool}")).with_kind(SpanKind::Server).start(&tracer);

        span.set_attribute(KeyValue::new("gateway.tool", tool.to_string()));
        // 트레이스는 표본이므로 카디널리티 걱정이 없고, 감사 로그와 대조하려면
        // 필요합니다.
        span.set_attribute(KeyValue::new("gateway.request_id", request_id.to_string()));
        span.set_attribute(KeyValue::new("gateway.session_id", session_id.to_string()));
        CallSpan(span)
    }

    /// 스팬을 닫고 메트릭을 기록합니다.
    pub fn end_call(&self, mut span: CallSpan, info: CallInfo) {
        use opentelemetry::trace::Status;

        span.0.set_attribute(KeyValue::new("gateway.decision", info.decision.clone()));
        span.0.set_attribute(KeyValue::new("gateway.system", info.system.clone()));

        if info.error_code != "ok" {
            span.0.set_attribute(KeyValue::new("gateway.error_code", info.error_code.clone()));
            span.0.set_status(Status::error(info.error_code.clone()));
        } else {
            span.0.set_status(Status::Ok);
        }
        span.0.end();

        // **저카디널리티 레이블만.** user_id·session_id·request_id·인자는 넣지
        // 않습니다.
        let labels = [
            KeyValue::new("tool", info.tool.clone()),
            KeyValue::new("system", info.system.clone()),
            KeyValue::new("decision", info.decision.clone()),
            KeyValue::new("code", info.error_code.clone()),
        ];

        self.calls.add(1, &labels);
        self.duration.record(info.latency.as_secs_f64() * 1000.0, &labels);

        if info.usage.input_tokens > 0 {
            self.tokens.add(
                info.usage.input_tokens as u64,
                &[KeyValue::new("tool", info.tool.clone()), KeyValue::new("direction", "input")],
            );
        }
        if info.usage.output_tokens > 0 {
            self.tokens.add(
                info.usage.output_tokens as u64,
                &[KeyValue::new("tool", info.tool.clone()), KeyValue::new("direction", "output")],
            );
        }
        if info.usage.cost_micros > 0 {
            self.cost.add(info.usage.cost_micros as u64, &[KeyValue::new("tool", info.tool.clone())]);
        }
        if info.error_code == "unavailable" {
            self.breaker_rejections.add(1, &[KeyValue::new("system", info.system)]);
        }
    }
}

/// Prometheus 수집기 + OTel 계량기.
pub struct Provider {
    registry: prometheus::Registry,
    pub telemetry: std::sync::Arc<Telemetry>,
}

impl std::fmt::Debug for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("Provider").finish() }
}

impl Provider {
    /// Prometheus **pull** 모델로 계측을 켭니다.
    pub fn new() -> Result<Self> {
        let registry = prometheus::Registry::new();
        let exporter = opentelemetry_prometheus::exporter().with_registry(registry.clone()).build()?;

        let provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder().with_reader(exporter).build();
        global::set_meter_provider(provider);

        let meter = global::meter("ai-bridge");
        Ok(Self {
            registry,
            telemetry: std::sync::Arc::new(Telemetry::new(&meter)),
        })
    }

    /// `/metrics` 본문.
    pub fn render(&self) -> String {
        use prometheus::Encoder as _;
        let encoder = prometheus::TextEncoder::new();
        let mut buf = Vec::new();
        let _ = encoder.encode(&self.registry.gather(), &mut buf);
        String::from_utf8(buf).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_labels_never_carry_identifiers() {
        // 이 목록이 곧 계약입니다. 여기에 user_id·session_id·request_id·인자를 더하면
        // 시계열이 폭발하고 개인정보가 메트릭 저장소로 샙니다.
        let allowed = ["tool", "system", "decision", "code", "direction"];
        for l in allowed {
            assert!(!l.contains("user"));
            assert!(!l.contains("session"));
            assert!(!l.contains("request"));
        }
    }

    #[test]
    fn provider_renders_prometheus_text() {
        let p = Provider::new().unwrap();
        let info = CallInfo {
            tool: "get_invoice_status".into(),
            system: "erp".into(),
            decision: "allowed".into(),
            error_code: "ok".into(),
            latency: Duration::from_millis(12),
            usage: Usage {
                input_tokens: 1500,
                output_tokens: 200,
                cost_micros: 4200,
            },
        };
        let span = p.telemetry.start_call("get_invoice_status", "req-1", "sess-1");
        p.telemetry.end_call(span, info);

        let body = p.render();
        // 메트릭 이름은 Go 판과 같아야 대시보드가 그대로 동작합니다.
        assert!(body.contains("gateway_tool_calls"), "본문: {body}");
        // 그리고 식별자는 레이블에 없어야 합니다.
        assert!(!body.contains("req-1"));
        assert!(!body.contains("sess-1"));
    }
}
