# rust-ai-bridge — go-ai-bridge 포팅 설계

Go 원본: `../go-ai-bridge` (비테스트 20,440줄 / 26 패키지 / 바이너리 4개).
목표: **동작 동등성(behavioral equivalence)** — 도구 이름·스키마·정책 판정·오류 코드·해시·감사 기록·MCP 와이어 포맷이 Go 판과 동일해야 한다.

---

## 0. 전제와 범위

### 0.1 누락된 의존성 (중요)

Go 코드는 형제 모듈 `go-legacies`를 import 하지만 **이 워크스페이스에 없다**. 따라서 `go build ./...` 는 현재 실패한다.

```
internal/bootstrap/run.go   → legacies/systems  (AdaptersWith, Options, RebindFromInventory)
internal/bootstrap/docs.go  → legacies/adapter/docs (Retriever, Embedder, pgvector)
internal/auditcli/ops.go    → legacies/systems
```

결정: **Rust로 재구성한다.** 어댑터 계약(`internal/adapter`)·`config/systems.yaml`·README의 도구 표·정책 파일이 요구하는 표면을 근거로 6개 어댑터와 전송 계층을 복원한다. 근거가 있는 재구성이지 창작이 아니다 — 도구 11종의 이름·시스템·접근·위험 등급·민감도가 README 표에 고정되어 있고, 정책 규칙이 참조하는 인자명(`amount`, `quantity`, `item`, `customer_id`, `invoice_id`)과 출력 필드명(`mask_fields`/`redact_fields` 대상)이 `config/policies.yaml`에 고정되어 있다.

### 0.2 테스트 범위

핵심 테스트만 포팅한다: 게이트웨이 파이프라인 종단, 정책(RBAC/ABAC·의무·에이전트 스코프), 승인(TTL·단회성·자기승인 금지), 감사 해시 체인·보존, 워크플로(재개·보상·lease·낙관적 잠금), PII·인젝션, 그리고 **`storetest` 적합성 스위트 36개 불변식 전부**. 커버리지 채우기용 `coverage_*.go` 는 생략.

---

## 1. 크레이트 구조

Go 의 `internal/` 은 크레이트 외부에서 접근 불가라는 뜻이다. Rust 에서는 **단일 라이브러리 크레이트 + 4개 바이너리**가 가장 가까운 사상이며, 모듈 경로가 Go 패키지와 1:1 대응한다.

```
rust-ai-bridge/
  Cargo.toml            # workspace
  crates/
    ai-bridge/          # lib: Go internal/* 전체
      src/
        auth/           registry/    policy/     schema/
        pii/            injection/   inventory/  toolcatalog/
        adapter/        breaker/     transient/
        audit/          approval/    workflow/   eval/
        ratelimit/      budget/
        gateway/        mcpserver/   telemetry/  llm/
        app/            bootstrap/   ops/        console/
        auditcli/       evalcli/
        storetest/      # 적합성 스위트 (pub, 테스트에서 호출)
    legacies/           # 재구성: go-legacies 대응
      src/
        legacy/         # Transport: rest/soap/db/file
        adapter/        # erp crm ticket docs purchase refund
        systems/        # 조립 + RebindFromInventory
  src/bin/  또는 crates/*/src/main.rs
    gateway/ auditctl/ agentctl/ evalctl/
  config/   eval/   scripts/   compose.yaml
```

`legacies` 를 별도 크레이트로 두는 이유: Go 의 모듈 경계를 그대로 유지해 "게이트웨이 코어는 어댑터 구현을 모른다"는 의존 방향을 컴파일러가 강제하게 한다. `ai-bridge` 는 `legacies` 를 의존하지 **않는다**(역방향: `legacies` → `ai-bridge` 의 `adapter`/`registry`/`auth` 계약).

---

## 2. 의존성 매핑 (전부 해석 확인됨)

| Go | Rust | 버전 | 비고 |
|---|---|---|---|
| `modelcontextprotocol/go-sdk` | `rmcp` | 2.2.0 | 공식 Rust MCP SDK. `ServerHandler` 수동 구현으로 **동적 도구 목록** 지원(핫 리로드 필수). `ctx.extensions.get::<http::request::Parts>()` 로 HTTP 헤더 → 신원 해석 |
| `net/http` (콘솔·메트릭·프로브) | `axum` + `tower` | 0.8.9 | rmcp 의 streamable-http 는 tower 서비스라 axum 에 그대로 mount |
| `modernc.org/sqlite` + `pgx/v5` | `sqlx` | 0.9.0 | `sqlite` + `postgres` 피처. 하나의 쿼리 계층으로 두 백엔드 |
| `redis/go-redis/v9` | `redis` | 1.3.0 | Lua 스크립트(`INCR`+`EXPIRE`), `HINCRBY` |
| `santhosh-tekuri/jsonschema/v6` | `jsonschema` | 0.47.0 | Draft 2020-12, **format assertion 켜야 함** |
| `gopkg.in/yaml.v3` | `serde_norway` | 0.9.42 | `serde_yaml` 후속. `deny_unknown_fields` 로 policy 의 `KnownFields(true)` 재현 |
| `html/template` (자동 이스케이프) | `maud` | 0.27.0 | 컴파일타임 템플릿 + **자동 HTML 이스케이프**. 콘솔이 프롬프트 원문을 렌더링하므로 필수 |
| OTel + Prometheus exporter | `opentelemetry` 0.32 / `opentelemetry-prometheus` 0.32 / `prometheus` 0.14 | | |
| `flag` | `clap` 4.6 | derive |
| `crypto/rand`, `sha256` | `rand` 0.10, `sha2` 0.11 | |
| `time` | `chrono` 0.4.45 | RFC3339 직렬화 |
| `net/netip` (CIDR) | `ipnet` 2.12 | `network_zone` 판정 |
| `regexp` (RE2) | `regex` 1.13 | 동일 RE2 계열 — 인젝션·PII 패턴 그대로 이식 |
| goroutine | `tokio` 1.52 | |

---

## 3. Go → Rust 관용구 변환 원칙

| Go | Rust |
|---|---|
| `interface` (Store, Recorder, Adapter, Limiter…) | `#[async_trait] pub trait` + `Arc<dyn Trait + Send + Sync>` |
| `map[string]any` | `serde_json::Value` / `serde_json::Map<String, Value>` |
| `(T, error)` | `Result<T, Error>` (`thiserror`) |
| `errors.Is/As` 체인 | `thiserror` + `#[source]`, `err.chain().any(...)` 로 분류 |
| 제로값 검증 (`RiskUnspecified`) | `enum RiskLevel` 에 **제로값 없음** — 파싱 단계에서 `Option` → 없으면 등록 거부 |
| `nil` 수신자 안전 메서드 | `Option<&T>` 또는 `impl` 에서 기본값 반환 |
| `sync.RWMutex` 핫 리로드 | `arc_swap::ArcSwap` 또는 `RwLock<Arc<T>>` (읽기 경로가 압도적으로 많음) |
| `context.Context` 취소/타임아웃 | `tokio::time::timeout` + `CancellationToken` |
| `time.Now` 주입 (테스트 시계) | `trait Clock { fn now(&self) -> DateTime<Utc> }` |

---

## 4. 바이트 단위로 재현해야 하는 것 (포팅 정확성의 핵심)

이것들이 어긋나면 컴파일은 되지만 **감사·승인이 조용히 깨진다.**

### 4.1 감사 해시 체인

```go
integrityHash(prev, e) = sha256( json.Marshal(struct{ Prev string; Entry Entry }{prev, e}) )
```
- Go `encoding/json` 은 **구조체 필드를 선언 순서대로**, **맵 키는 사전순 정렬**로 직렬화한다.
- 따라서 Rust 에서: `Entry` 는 `#[derive(Serialize)]` 로 선언 순서 유지(serde 기본 동작이 Go 와 일치), `Input`/`Output` 은 `serde_json::Map`(BTreeMap 기반 = 정렬)을 쓴다. **`preserve_order` 피처를 켜면 안 된다.**
- 래퍼 키는 대문자 `"Prev"`, `"Entry"` (Go 필드명 그대로, 태그 없음).
- `Timestamp` 는 저장·해시 전에 RFC3339(초 정밀도)로 정규화된 뒤 직렬화된다.
- 첫 항목의 `prev` 는 `""`.

### 4.2 승인 지문

```go
Fingerprint = sha256( actor + "\x00" + tool + "\x00" + json.Marshal(args) )
```
`args` 정규화는 **Go 의 맵 키 사전순 정렬이 곧 canonical form** 이다. Rust 는 `serde_json::Map`(정렬됨)으로 직렬화. `args == nil` 은 `{}` 로 취급한 뒤 지문 계산.

### 4.3 워크플로 idempotency key

```
"{runID}:recovery-{RecoveryCount}:{stepName}"
```
`Recover()` 가 `Failed`/`Compensated` 에서 호출되면 `RecoveryCount++` 하고 `Completed` 를 **전부 비운다**(처음부터 재실행). `Waiting` 재개는 둘 다 하지 않는다.

### 4.4 워크플로 메타데이터 사이드카 — **설계 결정 필요**

Go 는 `DefinitionVersion`/`InputHash`/`CurrentStep`/`LeaseOwner`/`LeaseUntil`/`FencingToken`/`NextRunAt`/`RecoveryCount` 를 **별도 컬럼이 아니라 `values_json` 안의 `_workflow_*` 예약 키**로 저장한다(사용자 스텝 데이터와 한 blob 에 섞임).

**채택안: Go 스키마를 그대로 유지한다.** 이유 — 기존 Go 가 쓴 `audit.db`/워크플로 DB 를 Rust 게이트웨이가 그대로 읽을 수 있어야 마이그레이션 없이 교체 가능하고, `storetest` 적합성 스위트가 두 구현에 동일하게 걸린다. (더 깔끔한 대안인 "실제 컬럼으로 승격"은 스키마 비호환이라 보류.)

### 4.5 타임스탬프 정밀도

- 감사·승인·eval 테이블: **RFC3339(초)** — 저장 전·해시 전 절삭.
- `workflow_event.ts` 및 워크플로 lease/next_run_at 메타: **RFC3339Nano**.
이 구분을 틀리면 해시 재현과 적합성 테스트의 정확 시각 단언이 깨진다.

---

## 5. 게이트웨이 파이프라인 (가장 중요한 계약)

`Handle()` → `admit()` → `execute()` → `shape_output()`. 실제 코드 순서(문서 주석 순서와 다름)를 따른다:

| # | 단계 | 실패 시 오류 코드 | 감사 `decision` |
|---|---|---|---|
| A | 도구 조회 (allowlist) | `not_found` | `denied` |
| B | 레이트 리밋 (키 `{user_id}:{tool}`) | `rate_limited` | `denied` |
| C | 비용 상한 | `budget_exceeded` | `denied` |
| D | 입력 스키마 (Draft 2020-12) | `invalid_input` | `denied` |
| E | 정책 (RBAC → 에이전트 스코프 → ABAC) | `permission_denied` | `denied` |
| F | L4 고위험 차단 | `high_risk_blocked` | `denied` |
| G | 승인 관문 | `approval_rejected` / `approval_error` | `denied` |
| G′ | 미승인 → **dry-run** | (오류 아님) | `allowed` |
| H | **L3+ 사전 감사 (fail-closed)** | `audit_unavailable` | — |
| I | 어댑터 실행 (breaker·timeout·retry) | `timeout`/`unavailable`/`adapter_error` | `allowed` |
| J | 출력 스키마 | `invalid_output` | `allowed` |
| K | 의무 집행 (redact·max_rows) | — | |
| L | PII 마스킹 | — | |
| M | 간접 인젝션 탐지 | — | `allowed` |
| N | 감사 로그 | — | |

반드시 보존할 비자명 규칙:

1. **모든 종료 분기가 `record()` 를 정확히 한 번 호출한다.** 거부·오류·dry-run 전부 감사된다.
2. **`record()` 는 결과와 무관하게 `budget.Add(CostMicros)` 를 누적한다** — 거부된 호출의 LLM 비용도 이미 발생했기 때문.
3. **H 단계만 `record()` 의 반환값을 검사한다.** L3+ 실행 직전 감사 기록이 실패하면 실행을 중단(`audit_unavailable`). 나머지 단계의 감사 실패는 파이프라인을 막지 않는다.
4. **어댑터 실패는 `denied` 가 아니라 `allowed` 로 감사된다** — 인가는 통과했고 시스템이 아픈 것이므로. `denial_codes` 집합이 이 구분을 만든다(메트릭 `decision` 레이블에 그대로 반영).
5. **간접 인젝션 스캔은 마스킹 전 `raw_output` 을 본다** — 마스킹이 신호를 지우지 못하도록.
6. **인젝션 탐지는 파이프라인을 막지 않지만 출력을 격리한다**: `Error` 를 반환하지 않고 `decision="allowed"` 를 유지한 채, `Data` 를 `{quarantined:true, reason, patterns, request_id}` 로 **치환**한다. (README 의 "표시만 한다"보다 강한 실제 동작 — 코드를 따른다.)
7. **쓰기 도구는 재시도하지 않는다**(`Access==Write` → `attempts=1`). 레지스트리가 `MaxRetries>0` 인 쓰기 도구 등록을 애초에 거부한다.
8. **정책: 첫 `deny` 규칙이 즉시 이긴다**(short-circuit). `allow_with_obligations` 는 전부 누적 병합(필드=합집합, `max_rows`=0 아닌 값들의 최솟값). `require_approval` 은 첫 규칙 ID만 사유로 기록하되 순회는 계속.
9. **부정형 술어는 결측값에 대해 참**(`not_equals`/`not_in`/`not_in_attribute`) — 속성 없는 주체를 "사내망 아님"으로 보고 거부(fail-closed). 반면 숫자 비교·`equals`·`in` 은 결측 시 발동하지 않음.

---

## 6. 신원 해석 (이중 신뢰 모델)

```
Bearer 토큰 있음  → sha256 해시로 조회. 없거나 만료면 거부. 헤더는 무시.
Bearer 토큰 없음  → X-User-Id 헤더로 조회.
                    조회된 주체가 kind: agent 면 → 거부 (헤더만으로 에이전트 사칭 불가)
```

`Enricher` 가 **매 요청** 다음 5개 속성을 계산해 **덮어쓴다**(주체가 주장 불가):
`business_hours`, `network_zone`, `request_time`, `llm_destination`, `business_purpose`.

- `network_zone`: 출발지 IP 를 `-internal-cidr` 과 대조. IP 불명 → `external`(fail-closed). `X-Forwarded-For` 는 `-trust-forwarded-for` 일 때만 신뢰하고, 기본은 미들웨어가 매 요청 덮어쓰는 `X-Gateway-Remote-Addr`(실제 peer).
- `business_hours`: 주말 제외 + `[start, end)`. `start==end==0` 이면 제한 없음.

---

## 7. 저장소 계층

| 트레이트 | 단일 노드 | 분산 |
|---|---|---|
| `AuditRecorder`/`Reader`/`Purger` | SQLite | PostgreSQL |
| `ApprovalStore` | SQLite | PostgreSQL |
| `WorkflowStore`/`Lister` | SQLite, Memory | PostgreSQL |
| `RateLimiter` | Memory | Redis (Lua `INCR`+`EXPIRE`) |
| `BudgetTracker` | Memory | Redis (`HINCRBY`) |

- `-postgres-dsn` 하나로 **세 저장소가 함께** 전환된다(갈라지면 분산에서 한쪽만 공유됨).
- 동시성: SQLite 는 `_txlock=immediate`(승인·워크플로), PostgreSQL 은 `SELECT … FOR UPDATE`(승인) / `pg_advisory_xact_lock(74201931)`(감사 체인 꼬리) / `ON CONFLICT … WHERE version=?`(워크플로 낙관적 잠금).
- **Redis 는 fail-open**(리소스 가드가 죽었다고 서비스를 멈추지 않음). **인가 가드(정책·승인)는 fail-closed.**
- 감사 삭제는 **export-then-delete**, 배치 500. 내보내기 실패 시 그 배치는 남고 Purge 중단. `Exporter` 는 **필수**(안 쓰려면 `Discard` 를 명시).

`storetest` 적합성 스위트(36개 불변식)를 Rust 로 포팅해 SQLite·Memory·PostgreSQL 구현에 **동일하게** 건다. 대표 불변식:
- 승인은 정확히 한 번만 소비된다(동시 8회 중 1회만 `approved`).
- 승인 TTL 시계는 **결정 시점**부터 흐른다.
- 만료 경계는 배타적(정확히 TTL 시점은 아직 유효).
- 요청자는 자기 요청을 결정할 수 없다(저장소가 트랜잭션 안에서 거부).
- 아카이브가 실패하면 감사 기록을 지우지 않는다.
- 오래된 버전으로 워크플로를 저장하면 거부되고, 동시 저장 중 정확히 하나만 성공한다.
- 동시 `Allow` 50회가 한도 10을 넘지 않는다.

---

## 8. MCP 서버

`rmcp` 의 `ServerHandler` 를 **수동 구현**한다(매크로 방식은 컴파일타임 고정이라 핫 리로드 불가).

- `list_tools` → 레지스트리 스냅샷을 `policy.visible(id, spec)` 으로 필터(RBAC + 에이전트 스코프만; ABAC 아님 — 이건 광고 축소일 뿐 보안 경계가 아니고, 실제 집행은 호출 시 `evaluate`).
- 위험 등급 → MCP 표준 annotation: `readOnlyHint = (access==read)`, `destructiveHint = (risk >= L3)`, `openWorldHint = false`, `idempotentHint = readOnly`.
- `call_tool` → `_meta` 키 4개를 읽는다(없어도 거부하지 않음 — 관측은 통제가 아님):
  `ai.bridge/prompt`, `ai.bridge/input_tokens`, `ai.bridge/output_tokens`, `ai.bridge/cost_micros`
- 결과에 판단 필드를 붙인다: `_masked`, `_narrowed`, `_request_id`. **텍스트 콘텐츠와 structured content 양쪽에** 실어야 한다(모델이 실제로 읽는 것은 텍스트).
- 게이트웨이 오류는 **MCP 프로토콜 오류가 아니라 `isError: true` 인 툴 결과**로 돌려준다(LLM 이 보고 스스로 교정하도록). 구조화 필드: `error_code`, `error_message`, `fallback`.
- 전송: stdio(`-user` 로 주체 고정) / Streamable HTTP(요청마다 해석).

---

## 9. 운영 콘솔

axum + maud. **인증 없이는 뜨지 않는다** — `Console::new` 는 리졸버가 없으면 오류(실수로 열어둘 방법이 없음). `admin` 역할 요구(`-console-role`).

- **CSRF**: 헤더 인증은 CSRF 를 막지 못하므로 POST 에 동일 출처 검사. `Sec-Fetch-Site` 우선(브라우저가 붙이며 위조 불가), 없으면 `Origin` vs `Host`. 둘 다 없는 POST 는 거부.
- **결정자는 폼이 아니라 세션에서** — 승인/거부 버튼은 `-by` 를 받지 않는다. 실제 차단은 승인 저장소가 한다.
- maud 가 프롬프트 원문·오류 메시지를 자동 이스케이프한다(안 하면 프롬프트 인젝션이 관리자 브라우저까지 이어짐).
- 화면: 대시보드 / 도구 호출 이력 / 인젝션 의심 / 사용량·비용 / 승인 / 업무 흐름 / 보존 기간 / 레거시 상태 / 인벤토리 / 에이전트 등록 / 품질 평가(`/eval`) / 설정·핫리로드(`/tools`).
- 프로브(인증 없음): `/livez` · `/readyz` · `/healthz`.

---

## 10. 계측

메트릭 이름은 Go 와 동일하게 유지:
`gateway.tool.calls`, `gateway.tool.duration`(ms), `gateway.llm.tokens`, `gateway.llm.cost_micros`, `gateway.breaker.rejections`.

**레이블 금지**: user_id · session_id · request_id · 도구 인자 — 카디널리티 폭발 + 메트릭 저장소로 PII 유출.
**스팬 속성에는 넣는다**: 트레이스는 표본이고 시계열이 아니며, 감사 로그와 대조하려면 필요.
`decision` 레이블이 "정책이 제대로 동작한 것(denied)"과 "시스템이 아픈 것(unavailable/timeout)"을 구분한다.

---

## 11. 재구성하는 legacies 계층

`config/systems.yaml` 6개 시스템 + README 도구 표 11종을 만족시킨다.

| 도구 | 시스템 | 접근 | 위험 | 승인 TTL |
|---|---|---|---|---|
| `get_invoice_status` | erp | read | L1 | — |
| `get_customer_invoices` | erp | read | L1 | — |
| `get_customer_profile` | crm | read | L1 | — |
| `search_contracts` | crm | read | L1 | — |
| `get_ticket_status` | ticket | read | L1 | — |
| `search_documents` | docs | read | L1 | — |
| `draft_purchase_request` | purchase | write | **L2** | (규칙으로 상향 가능) |
| `create_support_ticket` | ticket | write | **L3** | 4h |
| `submit_purchase_request` | purchase | write | **L3** | 24h |
| `get_workflow_status` | refund | read | L1 | — |
| `process_refund` | refund | write | **L4** | 15m |

- **Transport 추상화**: `trait Transport { call, health, describe }` — REST · SOAP · DB · File. ERP 어댑터는 **같은 코드**로 REST/SOAP/DB 를 쓴다. 오류 번역만 전송마다 다름(REST: 5xx/429/네트워크=일시적, 404=업무오류 / SOAP: `soap:Server`=일시적, `soap:Client`+"not found"=업무오류).
- **RAG**: `trait Retriever { index, search(q, allow) }` — 권한 필터(`allow` 술어)는 **검색기 밖에 있고 점수 매기기 전에** 적용된다(뒤에 걸면 상위 K건이 전부 걸러져 빈 결과가 됨). keyword(기본) / vector / pgvector.
- **행 수준 접근 제어**: 핸들러가 `Identity` 를 받는다. 볼 수 없는 문서는 "권한 없음"이 아니라 **존재하지 않는 것처럼** 처리(건수·제목 자체가 정보 노출).
- **환불 워크플로 6단계**: `lookup_invoice → check_refundable → calculate_amount → create_draft(보상: 초안 삭제) → execute_refund(보상: reversed 표시) → notify_customer(보상: 알림 회수)`. run ID 는 송장 ID 에서 결정적으로 생성(`refund-INV-2026-0001`) → 재개·멱등.

---

## 11b. ops 파사드 · 콘솔 · CLI

### ops (공유 파사드)
콘솔과 CLI 가 `audit`/`approval`/`workflow`/`eval` 을 직접 import 하지 않도록 조회·결정·카탈로그를 모은 계층. `Deps` 에 트레이트 객체를 담고, 잠금은 **두 개**(`principal_mu`: 주체 파일 쓰기 / `config_mu`: 정책·인벤토리·카탈로그·주체 YAML 적용 + 번들 리로드).

**설정 적용은 전부 원자적 쓰기 + 실패 시 롤백**: 임시 파일 → `rename`. 리로드 실패하면 백업 바이트를 되돌리고 다시 리로드한다.

`ReloadConfigBundle` 의 롤백 캐스케이드(SIGHUP 등가):
```
1. policy    실패 → 즉시 중단 (되돌릴 것 없음)
2. inventory 실패 → policy.rollback()
3. catalog   실패 → inventory.rollback() + policy.rollback() + 카탈로그 파일 복원
4. principal 실패 → catalog 복원 + inventory.rollback() + policy.rollback()
```

프로브(`/livez`·`/readyz`·`/healthz`)는 **인증 미들웨어보다 먼저 매칭**된다(`MountProbes`). `/readyz` 는 정책·감사·레지스트리·**어댑터 헬스**를 반영하고 실패 시 503.

### 콘솔 (axum + maud)
- 라우트 20개: `/`(대시보드) `/calls` `/injection` `/stats` `/approvals`(+POST decide) `/workflows` `/retention` `/health` `/inventory` `/tools`(+POST: 카탈로그·인벤토리·정책·주체 YAML 적용, 번들 리로드, 클러스터 알림) `/agents`(+POST register/edit/rotate/remove) `/eval` `/eval/turns` `/eval/turns/{id}`(+POST rate) `/eval/runs`.
- **인증 순서**: POST 면 **동일 출처 검사 먼저**(인증보다 앞) → 401(리졸버 실패) → 403(역할 부족).
- **CSRF 방어는 CSRF 토큰이 아니라 헤더뿐**: `Sec-Fetch-Site == "same-origin"` 우선, 없으면 `Origin` 의 host vs `Host`, **둘 다 없으면 거부**.
- **CSP: `default-src 'none'; style-src 'unsafe-inline'`** — 스크립트 전면 금지. 콘솔에 `<script>` 태그를 하나도 넣지 않는다.
- 승인 결정자는 **세션에서**(`current(r).UserID`), 폼에 `by` 필드가 존재하지 않는다.
- 템플릿 헬퍼: `time`(`01-02 15:04:05`) `since` `dash` `pct` `won`(천단위 콤마) `truncate`(**룬 단위**).

### CLI
- `auditctl`: `log`(기본) `stats` `health` `inventory` `policy-check` `policy-simulate` `approvals` `approve` `reject` `retention` `workflows` `workflow-events` `workflow-recover` `workflow-cancel`. **`AUDITCTL_POSTGRES_DSN`** 환경변수 지원(플래그 우선). DSN 이 잡히면 `-db` 는 무시.
  - `health` 는 장애 시스템이 있으면 **exit 1**(헬스체크 스크립트용).
  - `retention` 은 기본이 **조회**. `-purge` 시 `-archive-dir`/`-syslog`/`-discard` 중 **정확히 하나** 필수.
  - `approve ID -by NAME` — Go `flag` 가 첫 비플래그에서 멈추므로 위치 인자를 수동으로 벗겨낸다. clap 은 자연히 처리됨.
  - `policy-simulate` 는 **게이트웨이와 같은 정책 엔진**을 dry-run 한다 → **Go↔Rust 정책 동등성 검증에 그대로 쓴다**(같은 YAML·주체·인자로 두 구현의 출력 라인을 diff).
- `evalctl`: `turns` `show` `rate` `suites` `run` `runs` `run-show` `judge` `judge-all`. `run` 은 `-fail-under`(기본 1.0) 미달 시 **exit 1**(CI 게이트).
- `agentctl`: `token`(1회 발급, 평문은 stdout 에만) `hash`. **평문 토큰은 어떤 파일에도 쓰지 않는다** — `token_sha256` 만 저장.

### Go 원본의 잠재 버그 (재현하지 않고 고침)
`html/template` 은 런타임 해석이라 아래 두 화면은 실제로 렌더링 시 실패한다. Rust 의 타입 검사 템플릿에서는 애초에 컴파일되지 않으므로 뷰 모델에 필드를 추가한다.
1. `ops::WorkflowRun` 에 `completed`·`updated_at`·`compensate_error` 추가 (템플릿이 참조하나 구조체에 없음).
2. `ops::EvalRunResult` 에 `trail` 추가 (템플릿의 `len .Trail` 이 참조하나 구조체에 없음).

또한 `evalcli` 의 `truncate` 는 **바이트 단위**라 한글 프롬프트를 UTF-8 경계 중간에서 자른다 → Rust 에서는 룬 단위로 고친다.

---

## 12. 구현 순서

의존 방향을 따라 아래에서 위로. 각 단계는 `cargo build` + 해당 테스트 통과 후 다음으로.

1. **스캐폴드** — 워크스페이스, 의존성 고정, `config/`·`eval/` 복사.
2. **기반 계층** (I/O 없음, 순수 로직): `transient` → `schema` → `pii` → `injection` → `registry` → `inventory` → `auth` → `policy` → `breaker` → `adapter`(계약) → `toolcatalog`.
3. **저장소 계층**: `audit` → `approval` → `workflow` → `ratelimit` → `budget` → `eval`, 그리고 `storetest` 적합성 스위트.
4. **legacies 재구성**: `legacy`(transport) → `adapter/*` 6종 → `systems`(조립·리바인드).
5. **게이트웨이**: `telemetry` → `gateway`(파이프라인) → `llm` → `mcpserver`.
6. **운영**: `app` → `ops` → `console` → `bootstrap`(플래그·검증·핫리로드).
7. **CLI**: `auditctl` · `agentctl` · `evalctl` + `eval/golden` · `eval/judge`.
8. **검증**: `cargo clippy` 무경고, 테스트 녹색, **실제 게이트웨이 기동해 MCP 도구 호출 종단 확인**(빌드·테스트만으로는 포팅 성공을 주장하지 않음).

---

## 13. 알려진 위험

| 위험 | 대응 |
|---|---|
| Go `encoding/json` 직렬화 순서 재현 실패 → 해시 불일치 | `serde_json` 기본(맵=정렬, 구조체=선언순). `preserve_order` 피처 금지. 초기에 Go 판과 해시 교차 검증 테스트 작성 |
| `legacies` 재구성이 원본과 다름 | 계약·정책·설정이 강제하는 표면(도구명·인자명·출력 필드)에 고정. 정책 규칙이 참조하는 필드가 없으면 `ValidateReferences` 가 기동을 막으므로 검증이 자동으로 걸림 |
| `rmcp` 2.x API 변동 | 버전 고정(`=2.2.0`) |
| sqlx 컴파일타임 쿼리 검증(DB 필요) | 런타임 쿼리(`sqlx::query`) 사용 — 오프라인 매크로 캐시 불필요 |
| SQLite/PG 동시성 모델 차이 | 프리미티브가 아니라 **`storetest` 가 단언하는 결과**를 만족시키면 됨 |
