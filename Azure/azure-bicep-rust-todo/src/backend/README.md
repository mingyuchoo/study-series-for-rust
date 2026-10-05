# Clean Architecture TODO Application

Rust 기반 Clean Architecture 원칙을 적용한 TODO 애플리케이션입니다.

## 주요 기능

- TODO 관리용 RESTful API
- Swagger UI를 통한 API 문서화 및 테스트
- SQLite 데이터베이스 (자동 스키마 초기화)
- Clean Architecture (의존성 역전 원칙)
- Actix-web 기반 비동기 처리
- JSON 직렬화/역직렬화
- UUID 기반 엔티티 식별
- 생성/수정 날짜 추적
- OpenAPI 3.1 스펙 지원

## 아키텍처

Clean Architecture 원칙에 따른 4개 계층:

- **Domain 계층** (`src/domain/`): 핵심 비즈니스 엔티티 및 규칙
- **Application 계층** (`src/application/`): 유스케이스 및 비즈니스 로직
- **Adapter 계층** (`src/adapters/`): 외부 인터페이스 (HTTP, 데이터베이스)
- **Infrastructure 계층** (`src/infrastructure/`): 프레임워크 설정 및 의존성 주입

자세한 아키텍처 정보는 [ARCHITECTURE.md](ARCHITECTURE.md)를 참조하세요.

## 주요 의존성

| 패키지 | 버전 | 설명 |
|--------|------|------|
| `actix-web` | 4.15.0 | 웹 프레임워크 |
| `actix-files` | 0.7.0 | 정적 파일 서빙 |
| `sqlx` | 0.9.0 | SQLite 데이터베이스 |
| `utoipa` | 6.0.0 | OpenAPI 문서 생성 |
| `utoipa-swagger-ui` | 10.0.1 | Swagger UI |
| `serde` / `serde_json` | 1.0.229 / 1.0.151 | JSON 직렬화 |
| `uuid` | 1.27.0 | UUID 생성 |
| `chrono` | 0.4.45 | 날짜/시간 처리 |
| `clap` | 4.6.7 | CLI 인자 파싱 |
| `tokio` | 1.53.2 | 비동기 런타임 |

## API 문서

### Swagger UI
서버 실행 후 아래 주소에서 API 문서를 확인할 수 있습니다:
- **Swagger UI**: `http://localhost:8080/swagger-ui/`
- **OpenAPI JSON**: `http://localhost:8080/api-docs/openapi.json`

### API 엔드포인트

- `GET /api/todos` - 전체 TODO 조회
- `POST /api/todos` - 새 TODO 생성
- `PUT /api/todos/{id}` - TODO 수정
- `DELETE /api/todos/{id}` - TODO 삭제

## 빠른 시작

### 사전 요구사항

- Rust 1.99.0 (`rust-toolchain.toml`에 고정)
- Cargo 패키지 관리자

### 설치 및 실행

```bash
# 프로젝트 확인
cargo check

# 테스트 실행
cargo test

# 개발 모드 실행 (기본 포트 8080)
cargo run

# 커스텀 포트로 실행
cargo run -- --port 8000

# 릴리스 빌드
cargo build --release
```

### 데이터베이스

SQLite를 사용하며 프로젝트 루트에 `todos.db` 파일이 자동 생성됩니다.

### API 테스트

```bash
# 전체 TODO 조회
curl -X GET http://localhost:8080/api/todos

# 새 TODO 생성
curl -X POST http://localhost:8080/api/todos \
  -H "Content-Type: application/json" \
  -d '{"title": "Rust 학습", "description": "Rust 프로그래밍 언어 학습하기"}'

# TODO 수정 ({id}를 실제 ID로 대체)
curl -X PUT http://localhost:8080/api/todos/{id} \
  -H "Content-Type: application/json" \
  -d '{"title": "고급 Rust 학습", "completed": true}'

# TODO 삭제
curl -X DELETE http://localhost:8080/api/todos/{id}
```

## 프로젝트 구조

```
src/
├── domain/              # 도메인 엔티티 및 비즈니스 규칙
├── application/         # 유스케이스 및 저장소 트레이트
├── adapters/            # HTTP 핸들러 및 데이터베이스 구현
├── infrastructure/      # 앱 설정 및 의존성 주입
├── lib.rs               # 라이브러리 진입점
└── main.rs              # 애플리케이션 진입점
```
