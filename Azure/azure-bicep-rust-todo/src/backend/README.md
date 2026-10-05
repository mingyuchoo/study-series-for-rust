# Rust TODO 백엔드

Actix-web REST API, SQLx 기반 SQLite 저장소, Swagger UI와 프런트엔드 정적 파일 제공을 담당합니다. 도메인·유스케이스·어댑터·인프라 계층으로 나뉘며 `TodoRepository` 트레이트를 통해 저장소 구현을 주입합니다.

## 실행 환경과 의존성

Rust `1.99.0`을 [rust-toolchain.toml](rust-toolchain.toml)에 지정했으며 edition은 `2024`입니다. 아래 값은 [Cargo.toml](Cargo.toml)의 버전 요구사항이며, 실제 해석 버전은 [Cargo.lock](Cargo.lock)을 따릅니다.

| 패키지 | 버전 요구사항 | 역할 |
| --- | --- | --- |
| `actix-web` / `actix-files` | `4.15.0` / `0.7.0` | HTTP 서버 / 정적 파일 |
| `sqlx` | `0.9.0` | SQLite 비동기 쿼리 |
| `utoipa` / `utoipa-swagger-ui` | `6.0.0` / `10.0.1` | OpenAPI / Swagger UI |
| `serde` / `serde_json` | `1.0.229` / `1.0.151` | JSON 직렬화 |
| `uuid` / `chrono` | `1.27.0` / `0.4.45` | UUID v4 / UTC 시각 |
| `clap` / `tokio` | `4.6.7` / `1.53.2` | 실행 인자 / 비동기 런타임 |
| `async-trait` | `0.1.92` | 비동기 저장소 트레이트 |

Swagger UI는 `vendored` 기능을 사용합니다. UI 자산은 의존성에 포함되므로 빌드 중 Swagger UI 배포 파일을 GitHub에서 따로 내려받지 않습니다.

## 빠른 시작

아래 명령은 `src/backend`에서 실행합니다.

```bash
cargo check --locked
cargo run --locked
```

서버는 기본적으로 **`0.0.0.0:8000`**에 바인딩됩니다.

- API: <http://localhost:8000/api/todos>
- Swagger UI: <http://localhost:8000/swagger-ui/>
- OpenAPI JSON: <http://localhost:8000/api-docs/openapi.json>

```bash
# 포트 변경 (-p 8080도 가능)
cargo run --locked -- --port 8080

# 지원하는 실행 인자 확인
cargo run --locked -- --help

# 릴리스 빌드 및 실행
cargo build --locked --release
cargo run --locked --release -- --port 8000
```

포트는 CLI 인자로 설정합니다. 현재 코드에는 `PORT`, `DATABASE_URL`, `.env`를 읽는 로직이 없습니다. Swagger UI의 서버 선택 목록은 `8080`, `8000` 순으로 고정되어 있으므로 API 실행 시 실제 서버 포트를 선택해야 합니다. `Makefile.toml`의 `run-with-swagger` 안내 문자열도 `8080`이지만, 기본 실행 포트는 `8000`입니다.

### 프런트엔드와 통합 실행

웹 UI를 제공하려면 먼저 프런트엔드를 빌드합니다. `src/backend`를 시작 위치로 합니다.

```bash
cd ../frontend
npx --yes pnpm@12.9.1 install --frozen-lockfile
npx --yes pnpm@12.9.1 run build
cd ../backend
cargo run --locked
```

이후 <http://localhost:8000>에서 UI를 확인합니다. `./wwwroot/index.html`을 루트 문서로 제공하며, 정적 파일이 없는 임의의 경로를 `index.html`로 대체하는 SPA fallback은 없습니다. Vite 개발 서버를 사용할 때는 백엔드 정적 파일 빌드 없이 API를 실행할 수 있습니다.

## 데이터베이스

[src/infrastructure/db.rs](src/infrastructure/db.rs)는 **프로세스의 현재 작업 디렉터리**에 있는 `todos.db`를 `mode=rwc`로 엽니다. 파일이 없으면 생성하며, 시작 시 `CREATE TABLE IF NOT EXISTS`로 다음 스키마를 준비합니다. 작업 디렉터리에 쓰기 권한이 필요합니다.

| 컬럼 | SQLite 타입 | 의미 |
| --- | --- | --- |
| `id` | `TEXT PRIMARY KEY` | UUID v4 문자열 |
| `title` | `TEXT NOT NULL` | 제목 |
| `description` | `TEXT` | 선택적 설명 |
| `completed` | `BOOLEAN NOT NULL DEFAULT FALSE` | 완료 상태 |
| `created_at` | `TEXT NOT NULL` | 생성 UTC 시각, RFC 3339 |
| `updated_at` | `TEXT NOT NULL` | 수정 UTC 시각, RFC 3339 |

기존 테이블의 구조를 변경하는 마이그레이션은 없습니다. 목록은 `created_at DESC`로 정렬됩니다. Compose에서는 `/app/todos.db`를 사용하지만 DB 영속 볼륨은 설정되어 있지 않습니다.

## API

| 메서드 | 경로 | 요청 본문 | 성공 응답 |
| --- | --- | --- | --- |
| GET | `/api/todos` | 없음 | `200`, TODO 배열 |
| POST | `/api/todos` | 필수 `title`, 선택 `description` | `201`, 생성된 TODO |
| PUT | `/api/todos/{id}` | 선택 `title`, `description`, `completed` | `200`, 수정된 TODO |
| DELETE | `/api/todos/{id}` | 없음 | `204`, 본문 없음 |

개별 TODO 조회용 HTTP GET 경로는 없습니다. `PUT`은 전달된 필드만 바꾸며 수정할 때 `updated_at`을 갱신합니다. `description`을 생략하거나 `null`로 전달하면 기존 설명이 유지되고, 빈 문자열은 그대로 저장됩니다. 서버에 제목의 공백·빈 문자열을 거부하는 검증은 없습니다.

응답 예시:

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Rust 학습",
  "description": "Actix-web API 살펴보기",
  "completed": false,
  "created_at": "2026-10-05T00:00:00Z",
  "updated_at": "2026-10-05T00:00:00Z"
}
```

수정·삭제 대상이 없으면 `404`와 `{"error":"Todo not found"}`를 반환합니다. 저장소 오류는 `500`과 작업별 `error` 메시지를 반환합니다. 잘못된 JSON이나 필수 필드 누락 등은 Actix-web의 JSON 추출 단계에서 처리되므로 모든 오류가 이 JSON 형식을 따르는 것은 아닙니다.

OpenAPI에 Bearer 보안 스킴이 선언되어 있으나, 실제 인증·권한 검사 미들웨어는 없습니다.

### 요청 예제 (Bash)

```bash
curl http://localhost:8000/api/todos

curl -X POST http://localhost:8000/api/todos \
  -H 'Content-Type: application/json' \
  -d '{"title":"Rust 학습","description":"Actix-web API 살펴보기"}'

# 생성 응답의 id로 {id}를 바꿉니다.
curl -X PUT 'http://localhost:8000/api/todos/{id}' \
  -H 'Content-Type: application/json' \
  -d '{"completed":true}'

curl -i -X DELETE 'http://localhost:8000/api/todos/{id}'
```

## 코드 구조

```text
src/
├── domain/entities/todo.rs                  # Todo, 요청·오류 모델, 생성·수정 규칙
├── application/use_cases/
│   ├── todo_repository.rs                  # 저장소 인터페이스
│   └── todo_use_cases.rs                   # CRUD 유스케이스
├── adapters/
│   ├── http/todo_handlers.rs               # HTTP 처리와 OpenAPI 경로 정의
│   └── persistence/sqlite_todo_repository.rs # SQLx 저장소 구현
├── infrastructure/
│   ├── app.rs                              # 라우팅, Swagger, 정적 파일, 서버
│   ├── config.rs                           # CLI 포트 설정
│   ├── db.rs                               # SQLite 연결과 테이블 초기화
│   ├── openapi.rs                          # OpenAPI 문서 구성
│   └── setup.rs                            # AppState와 의존성 주입
├── lib.rs                                  # 모듈 공개
└── main.rs                                 # DB 초기화 후 서버 시작
```

추가 구조 설명은 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)를 참고하세요. 도메인 모델은 현재 `serde`, `chrono`, `uuid`, `utoipa`를 사용합니다.

## 테스트와 정적 분석

```bash
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

[tests/api.rs](tests/api.rs)의 통합 테스트는 단일 연결 메모리 SQLite와 Actix 테스트 앱을 사용합니다. TODO 생성·조회·완료 변경·삭제·중복 삭제 시 404, UUID 파싱, 생성 시각의 저장·복원, OpenAPI 3.1 문서와 Swagger UI 응답을 검사합니다. 로컬 `todos.db`는 사용하지 않으며, 실제 TCP 서버나 정적 파일 제공은 이 테스트 범위에 포함되지 않습니다.

[Makefile.toml](Makefile.toml)에는 선택적으로 사용할 수 있는 cargo-make 태스크도 있습니다. `build`, `release`, `test`, `run`은 `format`을 거쳐 `check`와 `clippy`를 먼저 수행하며, `format`은 소스 포맷을 변경합니다. 위의 일반 Cargo 명령에는 cargo-make가 필요하지 않습니다.

Docker와 Azure 구성의 현재 상태는 [루트 README](../../README.md)를 참고하세요.
