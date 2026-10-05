# Clean Architecture - TODO Application

This project has been refactored to follow Clean Architecture principles with clear separation of concerns.

## Project Structure

```
src/
├── domain/              # 도메인 계층 (Domain Layer)
│   └── entities/        # 도메인 엔티티
│       ├── mod.rs
│       └── todo.rs      # Todo 엔티티 및 요청 구조체
│
├── application/         # 애플리케이션 계층 (Application Layer)
│   ├── use_cases/       # 비즈니스 로직
│   │   ├── mod.rs
│   │   ├── todo_repository.rs    # Repository 트레이트
│   │   └── todo_use_cases.rs     # Todo 유스케이스
│   └── mod.rs
│
├── adapters/            # 어댑터 계층 (Adapter Layer)
│   ├── http/            # HTTP 어댑터
│   │   ├── mod.rs
│   │   └── todo_handlers.rs      # HTTP 핸들러
│   ├── persistence/     # 영속성 어댑터
│   │   ├── mod.rs
│   │   └── sqlite_todo_repository.rs  # SQLite 구현체
│   └── mod.rs
│
├── infrastructure/      # 인프라 계층 (Infrastructure Layer)
│   ├── app.rs          # 애플리케이션 설정
│   ├── config.rs       # 설정 관리
│   ├── db.rs           # 데이터베이스 초기화
│   ├── setup.rs        # 의존성 주입
│   └── mod.rs
│
├── lib.rs              # 라이브러리 진입점
└── main.rs             # 애플리케이션 진입점
```

## Architecture Layers

### 1. Domain Layer (`src/domain/`)
- **Purpose**: 핵심 비즈니스 로직과 엔티티
- **Dependencies**: 외부 의존성 없음
- **Contents**:
  - `Todo` 엔티티: 비즈니스 규칙과 데이터 구조
  - 요청/응답 구조체: `CreateTodoRequest`, `UpdateTodoRequest`

### 2. Application Layer (`src/application/`)
- **Purpose**: 애플리케이션 비즈니스 로직과 유스케이스
- **Dependencies**: Domain layer만 의존
- **Contents**:
  - `TodoRepository` 트레이트: 데이터 접근 인터페이스
  - `TodoUseCases`: 비즈니스 로직 구현

### 3. Adapter Layer (`src/adapters/`)
- **Purpose**: 외부 시스템과의 인터페이스
- **Dependencies**: Application과 Domain layer 의존
- **Contents**:
  - HTTP 어댑터: REST API 엔드포인트
  - 영속성 어댑터: 데이터베이스 구현체

### 4. Infrastructure Layer (`src/infrastructure/`)
- **Purpose**: 프레임워크, 설정, 의존성 주입
- **Dependencies**: 모든 계층에 의존 가능
- **Contents**:
  - 애플리케이션 설정
  - 데이터베이스 연결
  - 의존성 주입 설정

## Key Benefits

1. **관심사의 분리**: 각 계층이 명확한 책임을 가짐
2. **테스트 용이성**: 각 계층을 독립적으로 테스트 가능
3. **유지보수성**: 변경사항이 다른 계층에 미치는 영향 최소화
4. **확장성**: 새로운 기능 추가가 용이
5. **의존성 역전**: 고수준 모듈이 저수준 모듈에 의존하지 않음

## Running the Application

```bash
# 개발 모드로 실행
cargo run -- --port 8000

# 릴리즈 빌드
cargo build --release
./target/release/backend --port 8000
```

## API Endpoints

- `GET /api/todos` - 모든 할일 조회
- `POST /api/todos` - 새 할일 생성
- `PUT /api/todos/{id}` - 할일 수정
- `DELETE /api/todos/{id}` - 할일 삭제