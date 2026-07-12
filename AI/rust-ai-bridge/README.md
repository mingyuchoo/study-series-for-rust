# rust-ai-bridge

레거시 IT 시스템과 LLM AI 에이전트를 안전하게 연결하는 **AI Integration Gateway** 중간 계층.
[`go-ai-bridge`](../go-ai-bridge) 를 Rust 로 포팅한 것입니다.

LLM이 레거시 시스템(ERP·CRM·그룹웨어·DB 등)을 직접 만지지 못하게 하고, 중간에 게이트웨이를
두어 **API 어댑터·권한 제어·데이터 정제·도구 호출·감사 로그·승인 워크플로**를 통합 담당합니다.
게이트웨이의 도구는 MCP(Model Context Protocol) 서버로 노출되어 Claude Code/Desktop 등 외부
MCP 클라이언트가 사용할 수 있습니다.

핵심 원칙: **LLM은 판단하고, 게이트웨이는 집행한다.**

## 아키텍처

```
[MCP 클라이언트]  Claude Code/Desktop · 자체 오케스트레이터
        │  MCP Tools + Authorization / X-User-Id + _meta(프롬프트·토큰·비용)
        ▼
   Identity Resolver + Enricher   (이중 신뢰: 사람=헤더, 에이전트=Bearer 토큰)
        │
        ▼
┌──────────────────────────────────────────┐
│           AI Integration Gateway           │
│  레이트리밋 · 비용상한 · allowlist ·        │
│  입력검증 · 정책(RBAC+ABAC) · 고위험차단 ·  │
│  승인관문 · 어댑터실행 · 출력검증 ·          │
│  의무집행 · PII마스킹 · 인젝션표시 · 감사    │
└──────────────────────────────────────────┘
        │                              │
        ▼                              ▼
   Workflow Engine              Legacy Adapter Layer
   (lease·재개·보상)            erp·crm·ticket·docs·purchase·refund
                                      │
                                      ▼
                                Transport (REST·SOAP·DB·memory)
```

## 크레이트 구조

| 크레이트 | 역할 |
| --- | --- |
| `ai-bridge` | 게이트웨이 코어 — 정책·승인·감사·워크플로·MCP·콘솔·CLI 로직 |
| `legacies` | 레거시 어댑터 구현 + Transport (코어가 알지 못하는 계층) |
| `bridge-cli` | `gateway` · `auditctl` · `agentctl` · `evalctl` 실행 파일 |

`ai-bridge` 는 `legacies` 를 의존하지 **않습니다.** 어댑터 계약(`adapter::Adapter`)과
`AdapterFactory` 트레이트만 코어에 두고 바이너리가 구현을 주입합니다 — "게이트웨이는 레거시가
REST인지 SOAP인지 모른다"를 컴파일러가 강제합니다.

## 실행

```bash
# stdio (프로세스 하나가 사용자 한 명을 대신)
cargo run --bin gateway -- --user emp-sales-01 --allow-mock-backends

# Streamable HTTP + 운영 콘솔 + 메트릭
cargo run --bin gateway -- --http :8080 --console :8081 --metrics :9464 --allow-mock-backends

# 실제 REST 레거시로 연결 (목 백엔드 없이)
cargo run --bin gateway -- --erp-url https://erp.example.com

# 분산 배포: 세 저장소 → PostgreSQL, 두 카운터 → Redis
cargo run --bin gateway -- --http :8080 \
  --postgres-dsn 'postgres://user:pw@db:5432/gw?sslmode=disable' \
  --redis-url 'redis://cache:6379/0' --session-budget-micros 500000
```

`config/systems.yaml` 이 `memory`/빈 `base_url` 이면 `--allow-mock-backends` 없이는 기동을
**거부합니다.** 조용히 안전하지 않은 상태로 뜨지 않기 위함입니다.

### 관리자 CLI

```bash
auditctl log --db audit.db              # 도구 호출 이력
auditctl verify --db audit.db           # 감사 해시 체인 검증
auditctl approvals --db audit.db        # 승인 대기 목록
auditctl approve req_xxx --by manager-01   # 승인 (요청자 ≠ 결정자, 저장소가 강제)
auditctl retention --db audit.db        # 보존 기간 현황 (기본은 조회)
auditctl policy-check                   # 정책 참조 검증
auditctl policy-simulate --tool get_customer_profile --roles sales --attributes '{...}' --args '{...}'
auditctl health                         # 레거시 상태 (장애 시 exit 1)

agentctl token --user agent-support-bot # 에이전트 토큰 발급 (평문은 stdout 에만)

evalctl suites                          # 골든셋 목록
evalctl run --suite erp-read            # LLM 없이 게이트웨이에 도구 스크립트 (CI 게이트)
evalctl judge turn_xxx                  # 규칙 자동 채점 (--llm 으로 LLM judge)
```

`auditctl` 은 `AUDITCTL_POSTGRES_DSN` 환경변수를 봅니다 — 게이트웨이가 PostgreSQL 로 떴으면
CLI 도 같은 백엔드를 봐야 합니다.

## 포팅 정확성

이 포팅의 목표는 **동작 동등성**입니다. 특히 다음을 Go 판과 바이트 단위로 재현합니다.

- **감사 해시 체인** — `sha256(json({Prev, Entry}))`. Go `encoding/json` 의 구조체 선언순 +
  맵 키 사전순 정렬을 serde 기본 동작으로 재현합니다(`preserve_order` 금지).
- **승인 지문** — `sha256(actor \0 tool \0 json(args))`, 인자 키 정렬이 canonical form.
- **정책 판정** — deny-wins 단락, 의무 병합(필드=합집합·max_rows=0 아닌 값의 최솟값),
  부정형 술어의 fail-closed 결측 처리.
- **워크플로 멱등 키** — `{runID}:recovery-{n}:{step}`.

동등성 검증 수단: `auditctl policy-simulate` 가 게이트웨이와 **같은 정책 엔진**을 dry-run
하므로, 같은 YAML·주체·인자로 Go 판과 Rust 판의 출력을 diff 할 수 있습니다.

## 테스트

```bash
cargo test --workspace          # 349개 테스트
cargo clippy --workspace        # 무경고

# 분산 저장소 적합성 (백엔드가 있을 때만; 없으면 스킵)
TEST_POSTGRES_DSN='postgres://postgres:pw@localhost:5432/postgres?sslmode=disable' \
  cargo test -p ai-bridge --test conformance postgres
```

**저장소 적합성 스위트**(`storetest`)가 SQLite·Memory·PostgreSQL 구현에 **같은 불변식**을
겁니다 — 승인 단회성, TTL 결정 시점 시작, 요청자≠결정자, 아카이브 실패 시 무삭제, 낙관적
잠금 등. 여러 구현이 같은 검사를 통과한다는 것이 "교체 가능하다"의 뜻입니다.

**게이트웨이 파이프라인 종단 검사**(`legacies/tests/gateway_pipeline.rs`)는 실제
`config/*.yaml` 과 어댑터로 게이트웨이를 세우고 12단계를 검증합니다.

**골든셋**(`eval/suites/*.yaml`)은 LLM 없이 게이트웨이에 도구를 스크립트로 태워 회귀를 잡습니다.

상세 설계: [`docs/PORTING-DESIGN.md`](docs/PORTING-DESIGN.md).

## 기술 스택

- Rust 2024, `tokio`
- [`rmcp`](https://crates.io/crates/rmcp) 2.2 — MCP 서버 (`ServerHandler` 수동 구현으로 동적 도구 목록)
- `axum` + `maud` — 운영 콘솔 (자동 이스케이프, CSP `default-src 'none'`)
- `sqlx` — SQLite · PostgreSQL
- `redis` — 레이트 리밋 · 비용 상한 (Lua `INCR`+`EXPIRE`, `HINCRBY`)
- `jsonschema` — Draft 2020-12 (format 단언)
- `serde_norway` — 정책·주체·인벤토리 YAML
- `opentelemetry` + `prometheus` — 메트릭·트레이스

## 위험 등급과 노출 도구

승인 관문은 읽기/쓰기가 아니라 **위험 등급**에 걸립니다.

| 도구 | 시스템 | 위험 | 게이트웨이 동작 |
| --- | --- | --- | --- |
| `get_invoice_status` · `get_customer_invoices` | erp | L1 | 그대로 실행 |
| `get_customer_profile` · `search_contracts` | crm | L1 | 그대로 (정책 의무로 마스킹·행 제한) |
| `get_ticket_status` | ticket | L1 | 그대로 |
| `search_documents` | docs | L1 | 그대로 (권한 기반 RAG) |
| `get_workflow_status` | refund | L1 | 그대로 |
| `draft_purchase_request` | purchase | **L2** | 쓰기여도 승인 불필요 (초안은 효력 없음) |
| `create_support_ticket` | ticket | **L3** | 승인 필요 |
| `submit_purchase_request` | purchase | **L3** | 승인 필요 |
| `process_refund` | refund | **L4** | 기본 차단(`--allow-high-risk`) + 승인 필요 |
