# AMAP: Autonomous Modernization Assurance Platform

AMAP은 레거시 시스템의 기능을 증거에 기반해 재현하고 검증하는 Rust 기반 현대화 플랫폼입니다. 운영 동작 수집, 기능 동등성 비교, 비즈니스 규칙 지식 그래프, 불확실성 평가, 불변 증거 원장을 하나의 폐쇄 루프로 결합합니다. LLM은 교체 가능한 실행 엔진으로만 사용하며 최종 PASS 또는 FAIL은 결정론적 품질 게이트가 판정합니다.

구현의 출발점이 된 설계 문서는 [`docs/init.md`](docs/init.md)입니다.

```text
Control Plane (Axum, Cedar)
          |
          v
Orchestrator (DAG, NATS) --> Agents --> Verification Factory --> Quality Gate
          |                                |
          v                                v
Knowledge Graph                    Evidence Lake
(PostgreSQL, petgraph)              (Parquet, DataFusion, S3)
```

## 현재 구현 범위

AMAP은 다음 흐름을 실제 코드로 구현합니다.

```text
발견 -> 규칙 마이닝 -> 동작 마이닝 -> 불확실성 및 HITL
     -> 아키텍처 -> 빌드 -> 테스트, 경계값, 적대적 시나리오 생성
     -> 검증 -> 필요 시 RCA, 수정, 독립 리뷰, 재검증
     -> 기능 동등성 인증
```

- COBOL, Rust, JavaScript, C#, SQL, JCL 소스를 정적 분석합니다.
- 문서와 소스, 운영 추적을 결합해 비즈니스 규칙과 신뢰도를 계산합니다.
- 파일 또는 Kafka와 호환되는 Redpanda에서 운영 동작을 수집합니다.
- 정확 비교, 수치 허용 오차, 시간 허용 오차, 형식 및 고유성, 순서 무관, 무시 정책과 WASM 플러그인을 지원합니다.
- 정적, 단위, 골든 리플레이, 차등, 상태, 인터페이스, 경계값, 속성, 변이, 적대적, 장애, 동시성 검증 엔진을 제공합니다.
- 검증 실패 시 RCA, 수정, 독립 리뷰를 거쳐 설정된 횟수만큼 재검증합니다.
- PostgreSQL 지식 저장소, Parquet 증거 레이크, NATS JetStream 이벤트 버스, gRPC 검증 워커를 선택적으로 사용합니다.
- 모든 외부 LLM 요청에 역할별 라우팅, PII 마스킹, 캐시, 재시도, 토큰 예산, 비용 및 감사 로그를 적용합니다.

## 빠른 시작

### 사전 요구 사항

- Rust 1.98.0과 Cargo가 필요합니다. [`rust-toolchain.toml`](rust-toolchain.toml)이 `clippy`, `rustfmt`, `wasm32-unknown-unknown` 타깃을 지정합니다.
- 오프라인 대출 데모의 레거시 및 신규 시스템 에뮬레이터를 실행하려면 Python 3가 필요합니다.
- PostgreSQL, NATS, Redpanda, MinIO, Prometheus, Grafana를 함께 시험하려면 Docker Compose가 필요합니다.

### API 키 없이 데모 실행

저장소 루트에서 다음 명령을 실행합니다.

```bash
cargo run --locked --quiet --bin amap -- demo
```

이 명령은 인메모리 지식 저장소와 이벤트 버스, 로컬 Parquet 증거 레이크, fixture 기반 mock LLM을 사용합니다. 외부 서비스나 API 키가 필요하지 않습니다.

대출 데모는 [`examples/loan-demo`](examples/loan-demo)의 COBOL 중도상환 프로그램을 현대화합니다. 300개의 운영 추적으로 골든 마스터를 구성하고, 최초 구현에 포함된 ACT/365.25 계산과 수수료 하한 누락을 검출한 뒤 두 번의 수정 루프로 해결합니다. 12개 fail-closed 증거 기준과 Cedar 거버넌스 정책을 모두 통과해야 `FE CERTIFIED`가 됩니다.

전체 결과 JSON도 저장할 수 있습니다.

```bash
cargo run --locked --quiet --bin amap -- demo --out .amap/demo-outcome.json
```

### 전체 검증

```bash
./scripts/run.sh
```

이 스크립트는 워크스페이스 전체 타깃 빌드, Finance WASM 플러그인 빌드, 전체 테스트, 플러그인 테스트, 오프라인 데모를 순서대로 실행합니다. 개별 작업은 다음 명령으로 실행할 수 있습니다.

```bash
make architecture
make build
make test
make lint
make wasm
make demo
```

## 웹 운영 콘솔

`web/`은 OpenAPI 계약에서 TypeScript 타입을 생성하는 React 운영 콘솔입니다. 실행 명세 선택, 실행 시작, 에이전트 단계의 SSE 실시간 추적, HITL 승인 및 거절, 중단 실행 재개, 품질 게이트 확인을 한 화면에서 제공합니다.

로컬에서는 두 터미널을 사용합니다.

```bash
# 터미널 1: API 키가 필요 없는 로컬 Control Plane
AMAP_INSECURE_DEV=true AMAP_MOCK_LLM=true cargo run --bin control-plane

# 터미널 2: React 개발 서버
npm --prefix web install
npm --prefix web run dev
```

브라우저에서 `http://127.0.0.1:5173`을 엽니다. Vite가 `/v1`, `/healthz`, `/metrics`, `/openapi.yaml` 요청을 Control Plane으로 전달합니다.

프로덕션 번들은 다음 명령으로 만듭니다.

```bash
npm --prefix web run build
cargo run --release --bin control-plane
```

`web/dist/index.html`이 존재하면 Control Plane이 정적 파일과 SPA 경로를 같은 출처에서 제공합니다. 위치는 `AMAP_WEB_DIST`로 변경할 수 있습니다. Docker 이미지도 React 빌드를 포함합니다.

API 계약의 원본은 [`openapi/amap.yaml`](openapi/amap.yaml)이며 `npm --prefix web run generate:api`가 [`web/src/api/schema.d.ts`](web/src/api/schema.d.ts)를 생성합니다. 실행별 실시간 이벤트는 `GET /v1/runs/{id}/events`의 `text/event-stream` 응답으로 전달됩니다.

## 실행 모드

### 1. 오프라인 또는 CI 모드

`amap demo` 또는 `amap run --mock`을 사용합니다. run spec의 `[mock].fixtures`에 있는 응답을 읽으므로 네트워크와 LLM API 키가 필요하지 않습니다.

```bash
cargo run --locked --bin amap -- \
  run --spec examples/loan-demo/amap.toml --mock --out .amap/outcome.json
```

### 2. 내장 LLM 게이트웨이 모드

`AMAP_LLM_GATEWAY_URL`을 설정하지 않으면 CLI와 Control Plane이 프로세스 내부에 LLM 게이트웨이를 구성합니다. 다음 공급자 중 하나 이상을 설정합니다.

```bash
# Anthropic
export ANTHROPIC_API_KEY='...'
export AMAP_ANTHROPIC_MODEL='claude-opus-5'  # 생략 시 코드의 기본값을 사용합니다.

# OpenAI 호환 공급자
export OPENAI_API_KEY='...'
export AMAP_OPENAI_MODEL='배포된-모델-ID'    # 필수입니다.

cargo run --locked --bin amap -- run --spec /path/to/amap.toml
```

두 공급자가 모두 있으면 Builder와 Fix, RCA는 OpenAI를 우선하고 Discovery, Rule Miner, Architecture, Test Generator, Adversarial, Reviewer는 Anthropic을 우선합니다. 한 공급자만 있으면 해당 공급자로 폴백합니다.

### 3. 서비스 모드

`docker compose up -d`는 PostgreSQL, NATS, Redpanda, MinIO, Prometheus, Grafana만 시작합니다. AMAP 서비스는 별도로 실행합니다.

```bash
docker compose up -d

export AMAP_DATABASE_URL='postgres://amap:amap@localhost:5432/amap'
export AMAP_NATS_URL='nats://localhost:4222'
export AMAP_API_TOKEN='충분히-긴-control-plane-토큰'
export AMAP_LLM_GATEWAY_URL='http://127.0.0.1:8090'
export AMAP_LLM_GATEWAY_TOKEN='별도의-충분히-긴-gateway-토큰'
export AMAP_WORKER_TOKEN='별도의-충분히-긴-worker-토큰'
export ANTHROPIC_API_KEY='...'

cargo run --locked --bin llm-gateway
cargo run --locked --bin control-plane
```

위 두 서비스는 각각 별도 터미널에서 실행합니다. PostgreSQL을 사용하면 시작 시 `migrations/`가 자동 적용됩니다. 로컬에서 인증과 TLS 없이 기능만 확인하려면 격리된 개발 환경에서만 `AMAP_INSECURE_DEV=true`를 사용하십시오.

Control Plane에서 실행을 시작하는 예시는 다음과 같습니다. `spec`은 `AMAP_SPEC_ROOT` 아래의 파일이어야 하며 run spec이 참조하는 소스와 명령은 `AMAP_WORKER_ROOT` 경계 안에 있어야 합니다. 운영용 run spec은 `auto_approve_hitl = false`로 설정해야 합니다. 번들 대출 예제는 데모 편의를 위해 이 값이 `true`이므로 보안 모드의 Control Plane이 그대로는 거부합니다.

```bash
curl -X POST 'http://127.0.0.1:8080/v1/runs' \
  -H "Authorization: Bearer ${AMAP_API_TOKEN}" \
  -H 'X-AMAP-Actor: operator' \
  -H 'Content-Type: application/json' \
  -d '{"spec":"path/to/amap.toml","mock":false}'
```

## CLI

모든 하위 명령은 전역 `--settings <PATH>` 옵션을 받습니다. 기본 설정 파일은 [`config/amap.toml`](config/amap.toml)입니다.

| 명령 | 용도 |
|---|---|
| `amap run --spec amap.toml [--mock] [--out outcome.json]` | 폐쇄 루프 현대화 워크플로를 실행합니다. |
| `amap demo [--out outcome.json]` | 내장 대출 예제를 mock LLM으로 실행합니다. |
| `amap compare --spec spec.yaml [--plugin name=file.wasm] expected.json actual.json` | 두 JSON 문서를 비교합니다. 차이가 있으면 종료 코드 1을 반환합니다. |
| `amap invariant rules.dsl data.json` | JSON 데이터에 비즈니스 불변식을 적용합니다. 위반 시 종료 코드 1을 반환합니다. |
| `amap gate certificate.json [--metrics metrics.json]` | 인증서와 기능 동등성 지표를 기본 임계값으로 평가합니다. 미인증 시 종료 코드 1을 반환합니다. |
| `amap evidence [--sql 'SELECT ...']` | 증거 레이크의 `evidence` 테이블을 DataFusion SQL로 조회합니다. |
| `amap graph snapshot.json` | 실행이 저장한 지식 스냅샷을 Graphviz DOT로 출력합니다. |
| `amap discover legacy/` | LLM 없이 레거시 소스 트리를 정적으로 분석합니다. |

`gate`에서 `--metrics`를 생략하면 인증서의 골든 리플레이 합계만 전체 기능 동등성 지표로 사용합니다. P0, P1, 운영 동작 지표는 0으로 남으므로 전체 기본 게이트 평가에는 metrics 파일을 함께 제공하는 편이 적합합니다.

Finance WASM 비교기 빌드 및 사용 방법은 [`plugins/finance/README.md`](plugins/finance/README.md)를 참고하십시오.

## run spec

[`examples/loan-demo/amap.toml`](examples/loan-demo/amap.toml)이 실행 가능한 전체 예제입니다. 모든 상대 경로는 run spec 파일이 있는 디렉터리를 기준으로 해석합니다.

```toml
[function]
id = "FN-LOAN-0001"
name = "Loan early repayment posting"
domain = "Loan"
priority = "P0"

[[requirements]]
id = "REQ-1203"
title = "Accrued interest ACT/365"
text = "Interest accrues on an ACT/365 basis."
confidence = 0.95

[run]
source_root = "legacy"
documents = ["docs/requirements.md"]
traces = "traces/production.jsonl"
workspace = ".workspace/next"
next_command = ["python3", "{workspace}/loan_service.py"]
legacy_command = ["python3", "legacy/legacy_emulator.py"]
comparator_specs = ["specs/loan-repayment.yaml"]
default_spec = "loan-repayment"
invariants = "specs/invariants.dsl"
max_fix_iterations = 3
max_mutants = 40
faults = ["db_timeout", "api_timeout", "mq_duplicate", "partial_commit", "slow_response"]
auto_approve_hitl = false

[mock]
fixtures = "mock"
```

주요 설정은 다음과 같습니다.

- `traces`는 이전 형식의 단일 JSON Lines 파일을 받습니다.
- `[[run.trace_sources]]`는 `type = "file"` 또는 `type = "kafka"`를 받습니다. Kafka에는 `brokers`, `topic`, `group_id`와 선택 항목인 `max_records`, `idle_timeout_ms`를 지정합니다.
- Kafka 수집은 `amap-agents/kafka` 기능이 필요합니다. 예를 들어 `cargo run -p amap-cli --features amap-agents/kafka --bin amap -- run --spec amap.toml`로 활성화합니다. TLS와 SASL 접속 값은 `AMAP_KAFKA_*` 환경 변수로 전달합니다.
- `next_command`와 `legacy_command`는 JSON Lines 프로토콜을 사용하는 하위 프로세스입니다. `{workspace}`와 `{source_root}` 자리표시자를 사용할 수 있습니다.
- `timing_tolerance_ms`를 설정하면 절대 지연 시간 회귀 한도를 게이트에 반영합니다. 생략하면 시간은 증거로만 기록합니다.
- `[run.thresholds]`에서 품질 게이트 임계값을 실행별로 덮어쓸 수 있습니다.
- `auto_approve_hitl = true`는 데모와 CI 전용이며, Control Plane은 `AMAP_INSECURE_DEV=false`일 때 이를 거부합니다.

## 설정과 환경 변수

설정은 기본값, TOML 파일, `AMAP_*` 환경 변수 순서로 병합되며 뒤의 값이 앞의 값을 덮어씁니다.

| 설정 또는 환경 변수 | 기본값 | 설명 |
|---|---:|---|
| `database_url`, `AMAP_DATABASE_URL` | 미설정 | PostgreSQL URL입니다. 미설정 시 인메모리 지식 저장소를 사용합니다. |
| `nats_url`, `AMAP_NATS_URL` | 미설정 | NATS URL입니다. 미설정 또는 연결 실패 시 인메모리 이벤트 버스를 사용합니다. |
| `lake`, `AMAP_LAKE` | `.amap/lake` | 로컬 디렉터리 또는 `s3://bucket/prefix` 형식의 증거 레이크입니다. |
| `llm_gateway_url`, `AMAP_LLM_GATEWAY_URL` | 미설정 | 독립 LLM Gateway 주소입니다. 미설정 시 내장 게이트웨이를 사용합니다. |
| `control_plane_listen`, `AMAP_CONTROL_PLANE_LISTEN` | `127.0.0.1:8080` | Control Plane 수신 주소입니다. |
| `llm_gateway_listen`, `AMAP_LLM_GATEWAY_LISTEN` | `127.0.0.1:8090` | LLM Gateway 수신 주소입니다. |
| `worker_listen`, `AMAP_WORKER_LISTEN` | `127.0.0.1:50051` | gRPC 워커 수신 주소입니다. |
| `token_budget`, `AMAP_TOKEN_BUDGET` | `0` | 실행별 LLM 토큰 한도입니다. 0은 무제한입니다. |
| `spec_root`, `AMAP_SPEC_ROOT` | `.` | Control Plane이 허용하는 run spec 루트입니다. |
| `web_dist`, `AMAP_WEB_DIST` | `web/dist` | Control Plane이 제공할 React 프로덕션 빌드 디렉터리입니다. |
| `worker_root`, `AMAP_WORKER_ROOT` | `.` | 소스, 작업공간, 지원 파일, 실행 명령의 파일시스템 경계입니다. |
| `worker_allowed_executables`, `AMAP_WORKER_ALLOWED_EXECUTABLES` | `python3,python,cargo,java,javac` | 워커가 실행할 수 있는 프로그램 basename 목록입니다. |
| `worker_artifact_max_bytes`, `AMAP_WORKER_ARTIFACT_MAX_BYTES` | `67108864` | 원격 워커 아티팩트 번들의 최대 크기입니다. |
| `json_logs`, `AMAP_JSON_LOGS` | `false` | JSON 구조 로그를 사용합니다. |
| `otlp_endpoint`, `AMAP_OTLP_ENDPOINT` | 미설정 | OpenTelemetry OTLP gRPC 주소입니다. |
| `insecure_dev`, `AMAP_INSECURE_DEV` | `false` | 로컬 개발에서만 인증 및 TLS 요구를 완화합니다. |

운영 모드에서는 용도가 다른 `AMAP_API_TOKEN`, `AMAP_LLM_GATEWAY_TOKEN`, `AMAP_WORKER_TOKEN`을 각각 설정하십시오. S3 또는 MinIO 자격 증명과 엔드포인트는 `AWS_*` 및 `AWS_ENDPOINT_URL` 환경 변수로 전달합니다.

LLM 관련 추가 환경 변수는 다음과 같습니다.

- Anthropic은 `ANTHROPIC_API_KEY`, 선택 항목인 `AMAP_ANTHROPIC_MODEL`, `ANTHROPIC_BASE_URL`을 사용합니다.
- OpenAI 호환 공급자는 `OPENAI_API_KEY`, 필수 항목인 `AMAP_OPENAI_MODEL`, 선택 항목인 `OPENAI_BASE_URL`을 사용합니다.
- 독립 LLM Gateway만 `AMAP_LOCAL_LLM_URL`과 `AMAP_LOCAL_LLM_MODEL`, 선택 항목인 `AMAP_LOCAL_LLM_KEY`로 로컬 OpenAI 호환 서버를 등록할 수 있습니다.
- 의미 검색은 `AMAP_EMBEDDING_URL`, 선택 항목인 `AMAP_EMBEDDING_API_KEY`, `AMAP_EMBEDDING_MODEL`을 사용합니다.
- Control Plane 전체를 mock 공급자로 시작하려면 `AMAP_MOCK_LLM=true`를 사용할 수 있습니다. 요청 본문의 `"mock": true`와 마찬가지로 `AMAP_INSECURE_DEV=true`인 경우에만 허용됩니다.

## Control Plane API

`GET /healthz`와 `GET /openapi.yaml`은 공개됩니다. 그 밖의 엔드포인트는 `AMAP_API_TOKEN`이 설정된 경우 Bearer 인증을 요구합니다. 변경 주체는 `X-AMAP-Actor` 헤더로 기록하며 생략 시 `api-client`를 사용합니다.

| 메서드와 경로 | 설명 |
|---|---|
| `GET /v1/specs` | 설정된 루트에서 실행 가능한 run spec 목록을 조회합니다. |
| `POST /v1/runs` | run spec으로 비동기 실행을 시작합니다. |
| `GET /v1/runs` | 실행 목록을 조회합니다. |
| `GET /v1/runs/{id}` | 실행 상태, 체크포인트, 결과를 조회합니다. |
| `GET /v1/runs/{id}/events` | 과거 이벤트를 재생한 뒤 새 이벤트를 SSE로 전달합니다. |
| `POST /v1/runs/{id}/resume` | 승인된 HITL 검토가 있는 중단 실행을 재개합니다. |
| `GET /v1/functions` | 비즈니스 기능 목록을 조회합니다. |
| `GET /v1/functions/{id}/{resource}` | `requirements`, `rules`, `behaviors`, `scenarios`, `decisions`, `evidence`, `certificate`를 조회합니다. |
| `GET /v1/reviews` | HITL 검토 요청을 조회합니다. |
| `POST /v1/reviews/{id}/decide` | `{"status":"approved"}` 또는 `{"status":"rejected"}`를 제출합니다. |
| `GET /v1/events` | 현재 이벤트 버스의 이력을 조회합니다. |
| `GET /v1/evidence/query?sql=SELECT...` | 읽기 전용 DataFusion SQL로 증거를 조회합니다. |
| `GET /v1/graph.dot` | 현재 지식 그래프를 Graphviz DOT로 반환합니다. |
| `GET /v1/llm/audit` | 내장 게이트웨이 감사 로그를 조회합니다. |
| `GET /metrics` | Prometheus 형식 지표를 반환합니다. |

독립 LLM Gateway는 `POST /v1/complete`, `GET /v1/audit`, `GET /metrics`, 공개 `GET /healthz`를 제공합니다. 보호된 경로는 `AMAP_LLM_GATEWAY_TOKEN`을 사용합니다.

## 원격 검증 워커

`AMAP_VERIFIER_ENDPOINT`를 설정하면 Orchestrator가 로컬 실행 대신 [`proto/amap.proto`](proto/amap.proto)의 gRPC 계약으로 검증을 위임합니다. run spec은 이 주소와 자격 증명을 덮어쓸 수 없습니다.

```bash
cargo run --locked --bin verification-worker
```

`verification-worker`는 12개 검증 종류를 모두 제공합니다. 분리 배포용으로 `replay-worker`, `comparator-worker`, `mutation-worker`, `fault-worker`, `concurrency-worker` 바이너리도 있습니다.

운영 환경의 워커는 다음 설정이 모두 필요합니다.

- 워커는 `AMAP_WORKER_TLS_CERT`, `AMAP_WORKER_TLS_KEY`, `AMAP_WORKER_TLS_CLIENT_CA`로 mTLS 서버를 구성합니다.
- 호출자는 `AMAP_VERIFIER_TLS_CA`, `AMAP_VERIFIER_TLS_CERT`, `AMAP_VERIFIER_TLS_KEY`, `AMAP_VERIFIER_TLS_DOMAIN`을 설정합니다.
- 양쪽은 `AMAP_WORKER_TOKEN` Bearer 토큰을 사용합니다.
- 워커는 자체 완결형 소스, 작업공간, 지원 파일 스냅샷만 받고 크기, 경로 중복, 상위 디렉터리 이동, 실행 파일 허용 목록, 인라인 코드 실행 인자를 검증합니다.
- 원격 평문 워커와 아티팩트 없는 요청은 `AMAP_INSECURE_DEV=false`에서 거부됩니다.

## 품질 게이트

게이트는 증거에서 인증서를 계산하고 다음 12개 조건을 모두 확인합니다. 분모가 0인 비율은 0으로 계산하므로 증거가 없으면 통과하지 못합니다.

| 기준 | 기본 임계값 |
|---|---:|
| 요구사항 커버리지 | 100% |
| 구현 존재 여부 | `true` |
| 핵심 비즈니스 규칙 커버리지 | 100% |
| P0 기능 동등성 | 100% |
| P1 기능 동등성 | 99.999% 이상 |
| 전체 기능 동등성 | 99.9% 이상 |
| 운영 동작 커버리지 | 99.9% 이상 |
| 비즈니스 규칙 커버리지 | 99.9% 이상 |
| 변이 검출률 | 99% 이상 |
| 설명되지 않은 차이 | 0건 |
| 미해결 P0 및 P1 결함 | 0건 |
| 잔여 불확실성 | 0.1% 이하 |

이 판정은 [`crates/domain/src/gate.rs`](crates/domain/src/gate.rs)의 순수 함수가 수행하며 LLM 출력으로 대체되지 않습니다. 전체 워크플로에서는 12개 기준을 통과한 뒤 Cedar의 `certify` 권한 검사도 통과해야 하며, 결과에는 13번째 `Governance Policy` 항목이 추가됩니다.

## 저장소 구조

| 경로 | 역할 |
|---|---|
| `crates/domain` | 규칙, 동작, 시나리오, 검증 결과, 증거, 게이트의 표준 도메인 모델입니다. |
| `crates/assurance` | 증거 집계, 인증서, 잔여 불확실성, 게이트 입력을 계산하는 순수 검증 코어입니다. |
| `crates/comparator` | 내장 비교 정책, YAML 명세, Wasmtime 플러그인을 제공하는 기능 동등성 엔진입니다. |
| `crates/invariant` | pest 기반 비즈니스 불변식 DSL 파서와 평가기입니다. |
| `crates/uncertainty` | 규칙 신뢰도, HITL 등급, 잔여 불확실성, 다중 모델 합의를 계산합니다. |
| `crates/policy` | 자기 승인 금지와 독립 검증 등 Cedar 기반 에이전트 거버넌스를 적용합니다. |
| `crates/knowledge` | PostgreSQL 및 인메모리 지식 저장소를 제공합니다. |
| `crates/graph` | 지식 스냅샷의 영향도, 계보, 취약 증거 규칙, DOT 투영을 제공합니다. |
| `crates/evidence` | 로컬 또는 S3 Parquet 증거 레이크와 DataFusion 조회 어댑터를 제공합니다. |
| `crates/context` | Tantivy 어휘 검색, 벡터 검색, PII 마스킹, 토큰 제한 ContextPack을 제공합니다. |
| `crates/replay` | JSON Lines 및 HTTP SUT 어댑터, 골든 리플레이, 차등, 상태, 불변식 검증을 제공합니다. |
| `crates/llm` | Anthropic, OpenAI 호환, 로컬, mock 공급자와 라우팅, 예산, 감사 기능을 제공합니다. |
| `crates/orchestrator` | 체크포인트 가능한 DAG 실행기, 재시도, HITL 중단 및 재개, NATS 버스, Tonic 계약을 제공합니다. |
| `crates/agents` | 발견부터 인증까지의 에이전트와 폐쇄 루프 워크플로를 구현합니다. |
| `crates/platform` | 설정과 저장소, 버스, 증거 레이크, 정책, LLM을 조립하는 공용 구성 루트입니다. |
| `crates/telemetry` | 구조 로그, OpenTelemetry, Prometheus 지표를 제공합니다. |
| `services/amap-cli` | `amap` CLI를 제공합니다. |
| `services/control-plane` | 실행, 지식, HITL, 증거, 그래프, 감사 REST API를 제공합니다. |
| `services/llm-gateway` | 독립 LLM 게이트웨이를 제공합니다. |
| `workers` | 전체 또는 검증 종류별 gRPC 워커를 제공합니다. |
| `plugins/finance` | 별도 WASM 워크스페이스인 예제 금융 비교기입니다. |
| `migrations`, `proto`, `config`, `deploy` | PostgreSQL 스키마, gRPC 계약, 기본 설정, 배포 예시입니다. |

의존성은 함수형 코어와 명령형 셸의 경계를 따릅니다. `make architecture`는 순수 코어 크레이트가 인프라 의존성이나 직접 부작용을 새로 획득하지 않았는지 검사합니다.

## 배포 참고 사항

- [`Dockerfile`](Dockerfile)은 `amap`, `control-plane`, `llm-gateway`, `verification-worker`를 포함하는 런타임 이미지를 만듭니다. 기본 진입점은 `control-plane`입니다.
- [`deploy/k8s/amap.yaml`](deploy/k8s/amap.yaml)은 클라우드 중립적인 최소 골격입니다. 실제 배포 전 Secret, S3 자격 증명, 서비스별 토큰, 워커 mTLS 인증서, 네트워크 정책, 영구 볼륨을 환경에 맞게 추가해야 합니다.
- [`deploy/prometheus.yml`](deploy/prometheus.yml)은 로컬 호스트의 Control Plane과 LLM Gateway 지표를 수집하는 개발용 설정입니다.

## 알려진 제한 사항

- 워크플로 체크포인트와 수동 재개는 프로세스 재시작 후에도 보존되지만 다중 노드 스케줄링, 리스, 중단 실행 자동 회수에는 별도의 내구성 워크플로 백엔드가 필요합니다.
- Kafka 수집은 실행별 레코드 수와 유휴 시간으로 제한됩니다. 장기 수집, 스키마 레지스트리 연동, CDC 정규화는 구현되어 있지 않습니다.
- 실제 임베딩 생성은 지원하지만 벡터 인덱스는 프로세스 로컬입니다. 영구 pgvector 인덱싱과 모델 버전 마이그레이션은 구현되어 있지 않습니다.
- 워커 아티팩트 전송은 인증되고 크기가 제한된 단일 스냅샷입니다. 매우 큰 저장소에는 객체 저장소 기반 콘텐츠 주소화 매니페스트가 필요합니다.
- 변이 연산자는 텍스트 기반입니다. 동등 변이 분류 정확도는 시나리오 설계와 검토에 의존합니다.
