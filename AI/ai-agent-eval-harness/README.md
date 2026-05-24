# ai-agent-eval-harness

Rust로 구현된 AI 에이전트 평가(Evaluation) 하네스입니다. 다양한 도메인 시나리오에서 AI 에이전트의 도구 호출 정확도, 성능, 회귀 여부를 자동으로 평가합니다.

## 특징

- **3종 프론트엔드**: CLI 서브커맨드, 대화형 TUI, 내장 HTTP 서버 + SPA
- **다중 도메인 지원**: 고객 서비스, 금융 등 YAML 기반 시나리오 정의
- **멀티턴 대화 추적**: 다중 턴 대화 컨텍스트 관리
- **폴트 인젝션**: 도구 실패 시나리오 테스트
- **LLM 기반 채점**: LLM Judge를 활용한 응답 품질 평가
- **골든셋 검증**: 기준 응답과의 비교 검증
- **리포트 비교**: 두 실행 결과를 비교하여 회귀를 감지
- **세분화된 HTTP API**: 에이전트 실행, 단일 도구 호출, 폴트 주입, 궤적 채점까지 웹에서 직접 실행
- **TDD 추적성**: PRD → SPEC → TC → 구현 함수까지 `@trace` 태그로 양방향 추적
- **Azure OpenAI 지원**: Azure OpenAI API를 통한 LLM 연동

## 빠른 시작

```bash
# 1) 빌드
cargo build

# 2) CLI로 벤치마크 실행 (Azure 설정 불필요)
cargo run -- run --eval-scenario customer_service --agent passthrough

# 3) 결과를 웹에서 조회 + 추가 실행
cargo run -- serve
# → http://127.0.0.1:8080 접속
```

## 프로젝트 구조

Cargo 워크스페이스로 구성되어 있으며, 12개의 전문화된 크레이트로 이루어집니다.

```
ai-agent-eval-harness/
├── crates/
│   ├── agent-models/            # 에이전트 인터페이스 및 모델 (BaseAgent 트레이트, 궤적)
│   ├── agent-core/              # LLM 클라이언트, Azure OpenAI 설정, PPA 에이전트
│   ├── data-scenarios/          # YAML 시나리오 로더 및 도메인 설정 모델
│   ├── domains/                 # 도메인별 도구 구현
│   │   ├── customer_service/    #   고객 서비스 도구 (분류, 환불, 에스컬레이션)
│   │   └── financial/           #   금융 도구 (단리/복리 계산, 거래 검증)
│   ├── execution/               # 실행 엔진 (HarnessRunner, 비교기, 에이전트 레지스트리)
│   ├── execution-tools/         # 기본 도구 프레임워크 (BaseTool, ToolRegistry, 파일 도구)
│   ├── execution-fault-injection/ # 폴트 인젝션 (실패 모드 시뮬레이션)
│   ├── execution-multi-turn/    # 멀티턴 대화 컨텍스트 관리
│   ├── scoring/                 # 궤적 평가기, 골든셋 검증기
│   ├── scoring-llm-judge/       # LLM 기반 채점 모델
│   ├── reporting/               # 궤적 로거, 컬러 콘솔 출력
│   └── eval-harness/            # CLI 바이너리 (main 진입점)
│       └── src/
│           ├── main.rs          # clap 서브커맨드 정의
│           ├── tui/             # ratatui 기반 대화형 TUI (PRD-001)
│           └── web/             # Axum HTTP 서버 + SPA (PRD-002 ~ PRD-004)
│               ├── mod.rs       #   AppState, build_router, run_server
│               ├── handlers.rs  #   조회 API + index.html 임베드
│               ├── api.rs       #   전체 평가 시나리오 실행/비교 API
│               ├── api_exec.rs  #   세분화된 실행 API (crates 4~10)
│               └── index.html   #   정적 SPA
├── eval_data/
│   ├── scenarios/               # 도메인별 시나리오 YAML
│   │   ├── customer_service.yaml
│   │   └── financial.yaml
│   └── golden_sets/             # 골든셋 기준 응답 JSON
│       ├── customer_service.json
│       └── financial.json
├── reporting_logs/              # 평가 리포트 JSON (자동 생성)
├── reporting_trajectories/      # 실행 궤적 JSON (자동 생성)
├── docs/                        # TDD 추적성 산출물
│   ├── prd/                     #   PRD-001 ~ PRD-004
│   ├── spec/                    #   SPEC-001 ~ SPEC-004
│   ├── registry/                #   파편화 레지스트리 (counters/entries/trace)
│   └── traceability-matrix.md   #   자동 생성 매트릭스
├── .tdd-config.json             # TDD 워크플로우 경로 설정
├── Cargo.toml                   # 워크스페이스 설정
├── Makefile.toml                # cargo-make 태스크
├── rust-toolchain.toml          # Rust 툴체인 (nightly)
└── .env.example                 # 환경 변수 템플릿
```

## 요구사항

- Rust nightly (rust-toolchain.toml에 지정된 버전)
- [cargo-make](https://github.com/sagiegurari/cargo-make) (선택, Makefile.toml 사용 시)
- Azure OpenAI 계정 (PPA 에이전트 사용 시)

## 설치

```bash
cargo build
```

릴리즈 빌드:

```bash
cargo build --release
```

## 환경 변수 설정

`.env.example`을 복사하여 `.env` 파일을 생성하고 값을 채우세요:

```bash
cp .env.example .env
```

```env
AZURE_OPENAI_ENDPOINT=https://your-resource.cognitiveservices.azure.com
AZURE_OPENAI_API_KEY=your-key
AZURE_OPENAI_API_VERSION=2024-12-01-preview
AZURE_OPENAI_DEPLOYMENT=gpt-5.4
AZURE_OPENAI_REGION=koreacentral
AZURE_OPENAI_TEMPERATURE=1.0
AZURE_OPENAI_MAX_TOKENS=4096
```

> PPA 에이전트를 사용하지 않는 경우(passthrough 에이전트만 사용) `.env` 파일 없이도 실행 가능합니다.

## 사용법

`eval-harness` 바이너리는 6개의 서브커맨드를 제공합니다.

| 명령 | 용도 |
|------|------|
| `list` | 도메인/시나리오 및 등록된 에이전트 목록 출력 |
| `run` | 평가 시나리오 실행 및 리포트 저장 |
| `report <file>` | 저장된 리포트 JSON을 컬러 콘솔로 렌더 |
| `compare <baseline> <current>` | 두 리포트 파일 비교 (또는 SPEC-021 의 `--baseline-task/--current-task`, `--agent --baseline-since/...` 옵션) |
| `backfill-results` | SPEC-021: 기존 `reporting_logs/`, `reporting_trajectories/` 파일을 SQLite DB 로 일회성 import |
| `tui` | 대화형 TUI 2-패널 뷰 (조회 전용) |
| `serve` | Axum HTTP 서버 + 브라우저 SPA |

> 모든 명령은 `cargo run -- <command>` 또는 릴리즈 빌드 후 `./target/release/eval-harness <command>` 형태로 실행합니다.

### 데이터 경로 설정 (`eval-harness.toml`)

평가 데이터는 **SQLite 단일 파일**(`eval_data/eval_harness.db`)에 저장되며, 기본 시드는 `crates/data-scenarios/seed/scenarios/*.yaml`, `crates/data-scenarios/seed/goldens/*.json` 로 **바이너리에 임베드**되어 있습니다(SPEC-017). 최초 기동 시 DB 가 없거나 테이블이 비어 있으면 내장 시드로 자동 적재되며, 이후 실행은 DB 를 재사용합니다. 시드 소스는 깃 diff 로 변경을 리뷰할 수 있도록 저장소에 유지됩니다.

**SPEC-021**: 에이전트 실행 결과(궤적·평가 로그) 도 SQLite v4 스키마(`trajectories`, `evaluations`)에 dual-write 됩니다. 기존 파일 출력(`reporting_trajectories/*.json`, `reporting_logs/*.json`) 은 외부 도구·CI 워크플로우 호환을 위해 그대로 유지되며, 웹 API `GET /api/trajectories`, `GET /api/reports` 는 DB 우선 조회 + 파일 폴백 방식으로 동작합니다. 집계 보고서(`evaluation_report_*.json`) 와 비교 결과는 PRD-021 의 범위 외이며 파일로 유지됩니다. 기존 디렉토리에 쌓여 있는 결과를 DB 로 옮기려면 `cargo run -- backfill-results` 를 실행하세요. 시계열 비교는 `cargo run -- compare --agent ppa --baseline-since 2026-01-01T00:00:00Z --baseline-until 2026-03-31T23:59:59Z --current-since 2026-04-01T00:00:00Z --current-until 2026-04-30T23:59:59Z` 형식으로 가능합니다.

DB 경로는 다음 4단계 우선순위로 해석됩니다 (높음 → 낮음):

1. **CLI 인자** — `--db-path`
2. **환경변수** — `EVAL_HARNESS_DB_PATH`
3. **설정 파일** — 프로젝트 루트의 `eval-harness.toml`
4. **내장 기본값** — `eval_data/eval_harness.db`

설정 파일 예시 (`eval-harness.toml`):

```toml
[data]
# SQLite DB 파일 경로. 상대 경로는 이 설정 파일이 위치한 디렉토리 기준.
db_path         = "eval_data/eval_harness.db"
# seed 소스 디렉토리 (DB 가 비어 있을 때만 사용).
scenarios_dir   = "eval_data/eval_scenarios"
golden_sets_dir = "/var/lib/eval/golden"

# PPA 에이전트 루프 파라미터 (SPEC-016, SPEC-020).
[evaluation]
max_iterations       = 5
early_stop_threshold = 3
# 도메인 auto-routing 의 pre-filter top-K. 0 이면 비활성 (모든 도메인 도구 노출).
# 1 이상이면 task_description 에서 키워드 매칭이 가장 많은 상위 K 개 도메인의 도구만
# LLM 에 노출합니다. 도메인이 많아져 컨텍스트 토큰이 부담되면 2~3 권장.
domain_router_top_k  = 0
```

설정 파일이 없으면 기존 동작과 동일하게 내장 기본값(CWD 기준)이 사용됩니다. desktop 앱은 워크스페이스 루트에서 동일한 설정 파일을 검색합니다.

**DB 재빌드**: seed 소스(YAML/JSON)를 수정한 뒤 DB 에 반영하려면 `eval_data/eval_harness.db` 파일을 삭제하세요. 다음 실행 시 최신 파일 내용으로 자동 재생성됩니다. (DB 파일은 `.gitignore` 에 포함되어 있습니다.)

자세한 명세는 `docs/spec/SPEC-015.md`, `docs/spec/SPEC-016.md`, `docs/spec/SPEC-017.md` 참조.

### 시나리오/골든셋 CRUD (SPEC-019)

DB 로 전환된 저장소 위에서 **시나리오·골든셋을 REST API 로 직접 관리**할 수 있습니다. 모든 쓰기는 `eval_data/eval_harness.db` 에만 반영되며 YAML/JSON 파일은 건드리지 않습니다 (seed 소스로만 의미).

**REST 엔드포인트**:

```
POST   /api/eval-scenarios/:domain               시나리오 생성
PUT    /api/eval-scenarios/:domain/:id           시나리오 수정
DELETE /api/eval-scenarios/:domain/:id           시나리오 삭제 (연결된 골든셋도 cascade 삭제)

POST   /api/golden-sets/:domain                  골든셋 엔트리 생성
PUT    /api/golden-sets/:domain/:scenario_id     골든셋 엔트리 수정
DELETE /api/golden-sets/:domain/:scenario_id     골든셋 엔트리 삭제
```

**HTTP 에러 매핑**:
- `409 Conflict` — 동일 `(domain, id)` 중복 생성
- `404 Not Found` — 존재하지 않는 엔트리 수정/삭제
- `400 Bad Request` — 필수 필드 누락, 잘못된 ID 포맷 (`^[A-Za-z0-9_-]+$`, 1~64자)
- `500 Internal` — DB 오류

**웹 UI 관리 탭**: 웹 클라이언트(`cargo run -- serve`)에 "관리" 탭이 추가되었습니다. 도메인별 시나리오/골든셋 목록을 조회하고 JSON 편집기로 생성/수정/삭제할 수 있습니다.

**주의사항**:
- SQLite 의 `PRAGMA foreign_keys = ON` 이 모든 커넥션에서 활성화되어 시나리오 삭제 시 골든셋이 FK cascade 로 자동 삭제됩니다.
- v1 DB 는 기동 시 자동으로 v2 스키마(FK 추가) 로 마이그레이션됩니다 (무손실 table-rename).
- YAML/JSON 파일로의 역동기화(export)는 지원하지 않습니다. DB 가 유일한 Source of Truth 입니다.

자세한 명세는 `docs/prd/PRD-019.md`, `docs/spec/SPEC-019.md` 참조.

### 동적 도메인 CRUD + 도구 카탈로그 (SPEC-022)

`customer_service`, `financial` 외에 새로운 도메인을 **런타임에 추가**할 수 있습니다. 도메인은 `domains` 테이블에, 소속 도구 메타데이터는 `domain_tools` 테이블에, 라우터 키워드는 `domain_keywords` 테이블에 저장되며, `agent-core::domain_router` 는 DB 에서 키워드를 lazy-load 합니다. 도메인 CRUD 이후 `invalidate_cache()` 가 호출되어 다음 `select_domains` 호출부터 즉시 반영됩니다.

**REST 엔드포인트**:

```
GET    /api/domains                 전체 도메인 요약 목록 (tools, keywords, scenario_count, is_bootstrap)
GET    /api/domains/:name           단일 도메인 상세
POST   /api/domains                 도메인 생성 ({name, description, tools[], keywords[]})
PUT    /api/domains/:name           도메인 수정 (도구/키워드 전량 교체)
DELETE /api/domains/:name           도메인 삭제 (cascade: tools/keywords/external_tools)

GET    /api/tools/catalog           등록된 Rust 도구 전체 카탈로그 (domain 별)
```

**부트스트랩 보호**: `financial`, `customer_service` 는 Rust 에 정적 컴파일된 도메인이므로 `DELETE` 시 `409 Conflict` 를 반환합니다. 설명/도구/키워드 업데이트는 허용됩니다 (라우터 튜닝 용도).

**웹 UI**: "도메인" 탭에서 도메인 생성/수정/삭제, 멀티-셀렉트 도구 피커, 줄바꿈 구분 키워드 에디터 제공.

### 외부 HTTP 도구 (SPEC-023)

DB 로 정의된 `HttpCallTool` 은 **Rust 재컴파일 없이** 새 외부 API 를 에이전트 도구로 노출합니다. `external_tools` 테이블에 URL/메소드/헤더/바디 템플릿을 저장하면 `PpaAgent::load_all_tools` 가 매 task 시작 시 `ToolRegistry` 에 자동 등록합니다. 바디 템플릿의 `{{key}}` 는 도구 호출 파라미터로 치환됩니다.

**REST 엔드포인트**:

```
GET    /api/external-tools                       전체 external 도구 목록
GET    /api/external-tools/:domain               도메인별 목록
POST   /api/external-tools/:domain               생성 ({name, description, method, url, headers_json?, body_template, params_schema, timeout_ms})
PUT    /api/external-tools/:domain/:name         수정
DELETE /api/external-tools/:domain/:name         삭제
```

**보안 — URL allowlist**: 환경변수 `EVAL_HARNESS_HTTP_TOOL_ALLOWLIST` 에 콤마 구분 prefix 목록을 지정하면 해당 prefix 로 시작하는 URL 만 등록 가능합니다. 미설정 시 모든 URL 허용 (개발용 기본값).

```bash
export EVAL_HARNESS_HTTP_TOOL_ALLOWLIST="https://api.internal.corp/,http://localhost:"
cargo run -- serve
```

**웹 UI**: "도메인" 탭 하위에 외부 도구 섹션이 있어 method/url/headers/body_template/params_schema/timeout_ms 를 폼으로 편집할 수 있습니다.

자세한 명세는 `docs/prd/PRD-022.md`, `docs/spec/SPEC-022.md`, `docs/prd/PRD-023.md`, `docs/spec/SPEC-023.md` 참조.

### 시나리오 목록 조회

```bash
cargo run -- list

# 시나리오 디렉토리 직접 지정
cargo run -- list --scenarios-dir eval_data/eval_scenarios
```

### 벤치마크 실행

```bash
# 기본 (passthrough 에이전트, 전체 평가 시나리오)
cargo run -- run

# 특정 평가 시나리오 실행
cargo run -- run --eval-scenario customer_service
cargo run -- run --eval-scenario financial

# PPA 에이전트로 실행
cargo run -- run --agent ppa --eval-scenario financial

# 출력 파일 및 디렉토리 지정
cargo run -- run --agent ppa --output report.json --output-dir reporting_logs
```

### 리포트 조회

```bash
cargo run -- report reporting_logs/<report-file>.json
```

### 리포트 비교 (회귀 감지)

```bash
cargo run -- compare baseline.json current.json

# 허용 임계값 지정 (기본 5.0%)
cargo run -- compare baseline.json current.json --threshold 3.0

# 비교 결과 파일 저장
cargo run -- compare baseline.json current.json --output comparison.json
```

### TUI 모드 (대화형)

시나리오 목록과 저장된 리포트를 한 화면에서 탐색할 수 있는 2-패널 TUI를 실행합니다.

```bash
cargo run -- tui

# 디렉토리 직접 지정
cargo run -- tui --scenarios-dir eval_data/eval_scenarios --reports-dir reporting_logs
```

**키 바인딩**

| 키 | 동작 |
|----|------|
| `↑` / `k` | 이전 항목 선택 |
| `↓` / `j` | 다음 항목 선택 |
| `Tab` | 시나리오 ↔ 리포트 패널 전환 |
| `q` / `Esc` | TUI 종료 |

> TUI는 현재 **조회 전용**입니다. 벤치마크 실행과 리포트 상세 뷰는 `run`/`report` 서브커맨드를 사용하세요.

### 웹 클라이언트 (HTTP 서버)

Axum 기반 내장 HTTP 서버를 띄우면 **7-탭 SPA**가 함께 제공되어, 브라우저에서 모든 REST API(조회·실행·비교·채점·폴트주입)를 직접 사용할 수 있습니다. `index.html`과 `help.html`은 바이너리에 임베드되며 **한국어/영어 다국어 토글**을 지원합니다 (헤더의 `[한][EN]` 버튼, 선택은 `localStorage`에 저장).

**탭 구성**

| 탭 | 기능 | 사용 API |
|----|------|----------|
| **Run** | 평가 시나리오 + 에이전트 + 출력 파일명 선택 후 실행 | `POST /api/run` |
| **Scenarios** | 도메인/시나리오 트리 탐색 + 단일 시나리오 실행 | `GET /api/list`, `GET /api/scenarios/:d/:id`, `POST /api/scenarios/:d/:id/run` |
| **Tools** | 도구 선택 + params JSON 편집 + Invoke/Fault 시뮬레이션 | `GET /api/tools`, `POST /api/tools/:n/invoke`, `POST /api/tools/:n/simulate-fault` |
| **Agents** | 에이전트 + (선택) 도메인 + task/env 입력 후 직접 실행. 도메인 지정 시 해당 도메인 도구(customer_service/financial)가 PPA 에이전트에 로드됨 | `POST /api/agents/:n/execute` |
| **Reports** | 리포트 조회 + 두 리포트 비교 | `GET /api/reports`, `POST /api/compare` |
| **Trajectories** | 저장된 궤적 목록/상세 + 채점 | `GET /api/trajectories[/:name]`, `POST /api/score` |
| **Goldens** | 전체 파일 조회 + 단일 엔트리 조회 | `GET /api/golden-sets[/:d/:sid]` |

```bash
cargo run -- serve

# 옵션 지정 (모든 기본값 명시)
cargo run -- serve --addr 127.0.0.1:8080 \
                   --scenarios-dir eval_data/eval_scenarios \
                   --reports-dir reporting_logs \
                   --golden-sets-dir eval_data/golden_sets \
                   --trajectories-dir reporting_trajectories
```

실행 후 브라우저에서 `http://127.0.0.1:8080` 접속.

**HTTP API**

| 메서드 | 경로 | 설명 |
|--------|------|------|
| GET | `/` | SPA (정적 HTML) |
| GET | `/help` | 사용안내 페이지 (SPA 헤더 **📖 사용안내** 버튼에서 새 탭으로 열림) |
| GET | `/api/scenarios` | 도메인/시나리오 목록 JSON |
| GET | `/api/reports` | 저장된 리포트 파일명 목록 |
| GET | `/api/reports/:name` | 리포트 JSON 원문 (경로 순회 차단) |
| GET | `/api/list` | 도메인/시나리오 + 에이전트 집계 (CLI `list` 대응) |
| GET | `/api/agents` | 등록된 에이전트 이름 배열 (`passthrough`, 옵션으로 `ppa`) |
| GET | `/api/tools` | 전체 도메인 도구 메타데이터 (`execution-tools::ToolRegistry::get_tools_metadata`) |
| GET | `/api/golden-sets` | `eval_data/golden_sets/` 하위 골든셋 파일 배열 |
| GET | `/api/scenarios/:domain/:id` | 단일 시나리오 상세. 없으면 404 |
| POST | `/api/run` | 평가 시나리오 실행. body `{"eval_scenario","agent","output"?}` → `{report, saved_to}` |
| POST | `/api/compare` | 리포트 비교. body `{"baseline","current","threshold"?,"output"?}` → `{result, saved_to}` |

**POST 요청 예시**

```bash
# 평가 시나리오 실행 (파일명 지정 저장, CLI `run --output` 대응)
curl -X POST http://127.0.0.1:8080/api/run \
  -H 'content-type: application/json' \
  -d '{"eval_scenario":"customer_service","agent":"passthrough","output":"my_report.json"}'

# 리포트 비교 + 저장 (CLI `compare --output` 대응)
curl -X POST http://127.0.0.1:8080/api/compare \
  -H 'content-type: application/json' \
  -d '{"baseline":"a.json","current":"b.json","threshold":5.0,"output":"cmp.json"}'

# 도메인 + 에이전트 집계 (CLI `list` 대응)
curl http://127.0.0.1:8080/api/list
```

> `POST /api/run`은 항상 aggregate report를 `reports_dir`에 저장하며, `output` 생략 시 `evaluation_report_<timestamp>.json` 기본 파일명을 사용합니다. `POST /api/compare`는 `output` 지정 시에만 파일로 저장합니다.
>
> `POST /api/run`과 `/api/compare`는 `tokio::task::spawn_blocking` 으로 실행되어 서버 블로킹을 방지합니다. 리포트 이름은 `reports_dir` 내부 파일만 허용(경로 순회 차단).

**세분화된 실행 API (PRD-004)**

평가 파이프라인의 개별 단위를 웹에서 실행할 수 있습니다.

| 메서드 | 경로 | 설명 |
|--------|------|------|
| POST | `/api/scenarios/:domain/:id/run` | 단일 시나리오 실행. body `{"agent":"..."}` → `EvaluationResult` |
| POST | `/api/agents/:name/execute` | 에이전트 직접 호출. body `{"task":"...","environment":{...},"domain":"customer_service"\|"financial"\|null}` → `Trajectory`. `domain` 지정 시 해당 도메인 도구가 로드된 상태로 실행 (FR-8) |
| POST | `/api/tools/:name/invoke` | 단일 도구 호출. body `{"params":{...}}` → 결과 JSON |
| POST | `/api/tools/:name/simulate-fault` | 폴트 주입된 도구 호출. body `{"params":{...},"config":FaultInjectionConfig}` |
| GET | `/api/golden-sets/:domain/:scenario_id` | 골든셋 엔트리 상세 (없으면 404) |
| POST | `/api/score` | 궤적 채점. body `{"trajectory":{...}}` → `EvaluationResult` |
| GET | `/api/trajectories` | 저장된 궤적 파일명 목록 (`reporting_trajectories/`) |
| GET | `/api/trajectories/:name` | 궤적 JSON 원문 (경로 순회 차단) |

**예시**

```bash
# 단일 도구 호출
curl -X POST http://127.0.0.1:8080/api/tools/classify_inquiry/invoke \
  -H 'content-type: application/json' \
  -d '{"params":{"inquiry_text":"환불 요청","customer_id":"C1"}}'

# 단일 시나리오 실행
curl -X POST http://127.0.0.1:8080/api/scenarios/customer_service/cs_001/run \
  -H 'content-type: application/json' \
  -d '{"agent":"passthrough"}'

# 에이전트 직접 호출 (도메인 없음)
curl -X POST http://127.0.0.1:8080/api/agents/passthrough/execute \
  -H 'content-type: application/json' \
  -d '{"task":"hello"}'

# 에이전트 직접 호출 + 도메인 도구 로드 (FR-8)
curl -X POST http://127.0.0.1:8080/api/agents/ppa/execute \
  -H 'content-type: application/json' \
  -d '{"task":"고객 C123이 환불 요청","domain":"customer_service"}'

curl -X POST http://127.0.0.1:8080/api/agents/ppa/execute \
  -H 'content-type: application/json' \
  -d '{"task":"100만원 연 5% 3년 단리 계산","domain":"financial"}'

# 저장된 궤적 조회
curl http://127.0.0.1:8080/api/trajectories
```

> 범위 외: 멀티턴 대화 API(`ConversationManager`) 및 PpaAgent 전체 평가 시나리오에 대한 폴트 주입 연동은 다음 PRD로 분리되었습니다.

> 웹 API는 인증/인가 기능이 없습니다. 외부 노출 시 리버스 프록시에서 보호하세요.

### 데스크톱 앱 (Tauri)

`desktop/` 디렉토리는 **Tauri 2.x 기반 크로스 플랫폼 데스크톱 래퍼**입니다. 내장 Axum 서버를 무료 로컬 포트에 기동한 뒤, 시스템 WebView 로 동일한 7-탭 SPA 를 로드합니다. 웹 버전과 **100% 동일한 기능**을 네이티브 윈도우에서 사용할 수 있습니다.

> 이 프로젝트는 워크스페이스에서 `exclude` 되어 있어, 기본 `cargo build` 는 Tauri 시스템 의존성이 없어도 성공합니다.

**시스템 의존성**

| OS | 필수 패키지 |
|----|------------|
| Ubuntu 22.04+ | `sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev libayatana-appindicator3-dev librsvg2-dev` |
| macOS | `xcode-select --install` (Xcode Command Line Tools) |
| Windows | WebView2 Runtime (Win11 기본 포함) + MSVC 빌드 도구 |

**Tauri CLI 설치**

```bash
cargo install tauri-cli --version "^2.0"
```

**개발 실행** (데스크톱 윈도우 자동 오픈)

```bash
cd desktop
cargo tauri dev
```

**릴리즈 빌드** (플랫폼 네이티브 바이너리 생성)

```bash
cd desktop
cargo tauri build
```

또는 워크스페이스 루트에서 `cargo-make` 로 플랫폼별 번들을 한 번에 생성할 수 있습니다 (PRD-012 / SPEC-012):

```bash
# Windows (MSI/NSIS, 타겟: x86_64-pc-windows-msvc)
cargo make desktop-release-windows

# Linux (Deb/AppImage, 타겟: x86_64-unknown-linux-gnu)
cargo make desktop-release-linux

# macOS (.app/.dmg, 타겟: universal-apple-darwin)
cargo make desktop-release-macos

# 세 플랫폼을 연속 실행
cargo make desktop-release-all
```

> 위 태스크는 `desktop/` 크레이트로 `cwd` 진입 후 `cargo tauri build` 를 호출합니다. 크로스 타겟 toolchain(mingw, osxcross 등)과 `cargo-tauri` CLI 가 설치되어 있어야 해당 타겟을 실제로 빌드할 수 있으며, `install_crate` 메커니즘이 `cargo-tauri` 를 자동 설치합니다.

> `desktop/icons/icon.png` 에는 32×32 플레이스홀더(#f0c419 단색)가 포함되어 있어 `cargo tauri dev` / `cargo build` 가 즉시 동작합니다. 릴리즈 번들용 실제 아이콘 세트가 필요하면 `desktop/icons/README.md` 의 `cargo tauri icon` 가이드를 참조하고 `tauri.conf.json` 의 `bundle.active` 를 `true` 로 변경하세요.

**아키텍처**

```
┌─────────────────────────────────────────────┐
│  Tauri Desktop Window (system WebView)      │
│  └─ loads http://127.0.0.1:<auto_port>      │
│       └─ 7-탭 SPA + 한/영 i18n 그대로 재사용 │
└──────────────────────┬──────────────────────┘
                       │
              ┌────────▼────────┐
              │ 내장 Axum 서버  │  (detached thread)
              │ (같은 프로세스) │
              └─────────────────┘
```

> 백엔드 변경 없이 웹 UI 를 네이티브 앱으로 감싸는 패턴입니다. 향후 Tauri IPC(`invoke()`) 기반 직접 호출로 전환하는 것은 별도 PRD 대상입니다.

## 에이전트

| 에이전트 | 설명 |
|----------|------|
| `passthrough` | 항상 빈 응답을 반환하는 기본 에이전트 (베이스라인용) |
| `ppa` | Azure OpenAI를 사용하는 PPA(Perceive-Policy-Action) 루프 에이전트 |

## 시나리오 도메인

### 고객 서비스 (`customer_service`)

| 시나리오 | 난이도 | 도구 |
|----------|--------|------|
| cs_001: 고객 문의 분류 | easy | classify_inquiry |
| cs_002: 환불 요청 처리 | medium | classify_inquiry, process_refund |
| cs_003: 불만 고객 에스컬레이션 | medium | classify_inquiry, escalate_issue |
| cs_004: 복합 고객 서비스 워크플로우 | hard | classify_inquiry, process_refund, escalate_issue |
| cs_005: 문의 데이터 파일 분석 | hard | read_file, classify_inquiry, write_file |

### 금융 (`financial`)

| 시나리오 | 난이도 | 도구 |
|----------|--------|------|
| fin_001: 단리 이자 계산 | easy | calculate_simple_interest |
| fin_002: 복리 이자 계산 | medium | calculate_compound_interest, calculate_simple_interest |
| fin_003: 대액 출금 검증 | medium | validate_transaction |
| fin_004: 예금 데이터 파일 분석 | hard | read_file, calculate_simple_interest, write_file |
| fin_005: 종합 금융 분석 | hard | calculate_simple_interest, calculate_compound_interest, write_file |

## 도메인 아키텍처 (SPEC-020)

에이전트는 **모든 도메인의 도구를 단일 네임스페이스로 동시에 보유**하며, LLM 의 function-calling 능력으로 task 에 맞는 도구(=도메인)를 스스로 선택합니다. 도구 이름은 `<domain>__<tool>` 형식으로 네임스페이스되어(예: `financial__calculate_simple_interest`, `customer_service__classify_inquiry`) 도메인 간 이름 충돌이 없습니다. `read_file`/`write_file`/`list_directory` 기본 파일 도구는 `general` 도메인으로 네임스페이스 없이 제공됩니다.

- **Stage 1 (Tool Fusion)**: `crates/domains/src/lib.rs::register_all()` 하나의 진입점으로 모든 도메인 도구 등록. 새 도메인 추가 시 이 함수에 1줄 추가.
- **Stage 2 (Routing Evaluation)**: 골든셋 엔트리에 `expected_domain` 필드를 선언하면, 평가 시 에이전트의 첫 tool call 도메인이 일치하는지 `domain_routing_score` (0.0/1.0) 로 측정됩니다.
- **Stage 3 (Keyword Pre-filter)**: 도메인이 많아져 토큰 비용이 문제가 되면 `eval-harness.toml` 의 `[evaluation] domain_router_top_k` 를 1+ 로 설정하여 `task_description` 키워드 매칭 상위 K 개 도메인만 LLM 에 노출할 수 있습니다. 0(기본값)이면 모든 도메인 공개.

## 커스텀 시나리오 추가

런타임에 시나리오를 추가/수정하려면 웹 UI 의 **Scenarios/Goldens** 탭 또는 CRUD API(`POST /api/eval-scenarios/:domain`, `PUT /api/eval-scenarios/:domain/:id` 등)를 사용하세요. 변경 내용은 SQLite DB 에 저장됩니다(SPEC-019).

새 도메인을 **기본 시드**에 포함시키려면:

1. `crates/data-scenarios/seed/scenarios/<domain>.yaml` 및 `crates/data-scenarios/seed/goldens/<domain>.json` 추가
2. `crates/data-scenarios/src/seed_embedded.rs` 의 `EMBEDDED_SCENARIO_YAMLS` / `EMBEDDED_GOLDEN_JSONS` 에 엔트리 등록
3. `crates/domains/src/<domain>/` 에 도구 Rust 코드 추가
4. `crates/domains/src/lib.rs::register_all()` 에 `register_<domain>(registry)` 호출 추가, `known_domains()` 배열에 이름 추가
5. (선택) `crates/agent-core/src/domain_router.rs::DOMAIN_KEYWORDS` 에 키워드 매핑 추가 (Stage 3 pre-filter 사용 시)

YAML 예시:

```yaml
name: my_domain
description: 나의 도메인 설명

tools:
  - class_name: MyTool
    module_path: domains.my_domain.tools

scenarios:
  - id: my_001
    name: 시나리오 이름
    description: 시나리오 설명
    task_description: >
      에이전트에게 전달할 태스크 설명
    initial_environment:
      key: value
    expected_tools:
      - my_tool
    success_criteria:
      result_key: expected_value
    difficulty: easy  # easy | medium | hard
```

## 개발

```bash
# 포맷 + 검사 + 린트 + 빌드
cargo make build

# 포맷 + 검사 + 린트 + 릴리즈 빌드
cargo make release

# 포맷 + 테스트
cargo make test

# 파일 변경 시 자동 재실행
cargo make watch-run

# 포맷만
cargo make format

# 린트만
cargo make clippy

# 데스크톱 릴리즈 번들 (Windows/Linux/macOS, SPEC-012)
cargo make desktop-release-windows
cargo make desktop-release-linux
cargo make desktop-release-macos
cargo make desktop-release-all
```

## 아키텍처

```
Frontends (eval-harness binary)
 ├── CLI           (clap 서브커맨드: list | run | report | compare | tui | serve)
 ├── TUI           (ratatui + crossterm, 2-패널 조회)
 └── Web server    (Axum + 임베드 SPA)
        ├── 조회 API  : /api/{scenarios,reports,agents,tools,golden-sets,trajectories}
        └── 실행 API  : /api/{run,compare,score}, /api/scenarios/:d/:id/run,
                        /api/agents/:n/execute, /api/tools/:n/{invoke,simulate-fault}

Core (workspace crates)
 └── execution::HarnessRunner
        ├── AgentRegistry           → BaseAgent (passthrough | ppa)
        ├── ToolRegistry            → Domain Tools (customer_service, financial)
        ├── scoring::TrajectoryEvaluator
        │    └── GoldenSetValidator
        ├── execution-fault-injection::FaultInjector
        ├── execution-multi-turn::ConversationManager
        └── reporting::TrajectoryLogger
              ├── reporting_logs/          (EvaluationResult / Report JSON)
              └── reporting_trajectories/  (Trajectory JSON)
```

## TDD 추적성

본 프로젝트는 PRD → SPEC → 테스트 케이스(TC) → 구현 함수까지 `@trace` 태그로 양방향 추적됩니다.

```
docs/prd/PRD-001.md  ──┐
docs/prd/PRD-002.md    │  docs/spec/SPEC-00{1..4}.md
docs/prd/PRD-003.md    │     │
docs/prd/PRD-004.md  ──┘     └─ TC → @trace (테스트/구현 함수)
                                    │
                                    └─ docs/traceability-matrix.md (자동 생성)
```

**PRD 목록**

| PRD | 내용 | SPEC |
|-----|------|------|
| PRD-001 | TUI 모드 (ratatui 2-패널) | SPEC-001 |
| PRD-002 | 웹 서버 기본 조회 API + SPA | SPEC-002 |
| PRD-003 | 전체 크레이트 능력 HTTP 노출 (agents/tools/goldens/run/compare) | SPEC-003 |
| PRD-004 | 세분화된 실행 API (단일 시나리오/에이전트/도구/채점/폴트/궤적) | SPEC-004 |
| PRD-005 | CLI↔Web 완전 동등화 (`/api/list`, `run/compare` 의 `output` 필드) | SPEC-005 |
| PRD-006 | 탭 기반 SPA UI — 7개 탭에서 전체 API 사용 | SPEC-006 |
| PRD-007 | `/help` 사용안내 페이지 + SPA 헤더 버튼 | SPEC-007 |
| PRD-008 | 한/영 다국어 토글 (헤더 버튼, localStorage 영속화) | SPEC-008 |
| PRD-009 | Tauri 데스크톱 앱 (`desktop/`, 내장 Axum 서버 + 시스템 WebView) | SPEC-009 |
| PRD-010 | IBM Plex Sans KR/Sans/Mono 타이포그래피 적용 | SPEC-010 |
| PRD-011 | select/option 다크 테마 스타일 (color-scheme, 커스텀 화살표) | SPEC-011 |
| PRD-012 | 데스크톱 앱 크로스 플랫폼 릴리즈 스크립트 (Makefile.toml) | SPEC-012 |
| PRD-014 | 웹 UI 라이트/다크 테마 토글 (CSS 변수 기반, localStorage 영속화) | SPEC-014 |

**추적성 검증**

```bash
# 추적성 검사만
python3 ~/.claude/skills/tdd-workflow/scripts/verify_trace.py

# 검사 + 매트릭스 재생성
python3 ~/.claude/skills/tdd-workflow/scripts/verify_trace.py --matrix
```

## 라이선스

BSD-3-Clause
