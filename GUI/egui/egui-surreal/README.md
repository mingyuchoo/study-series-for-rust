# SurrealDB GUI 애플리케이션 - Clean Architecture

Clean Architecture 원칙을 적용하여 egui로 구축한 SurrealDB GUI 애플리케이션입니다.

## 아키텍처 개요

애플리케이션은 네 개의 주요 계층으로 구성됩니다:

### 1. 도메인 계층 (`src/domain/`)

- **엔티티** (`entities.rs`): `Person`, `PersonData`, `AuthParams`, `MessageType`, `AppMessage` 등 핵심 비즈니스 객체
- **리포지토리 트레이트** (`repositories.rs`): 데이터 접근 추상 인터페이스 (`PersonRepository`, `AuthRepository`, `QueryRepository`)

### 2. 애플리케이션 계층 (`src/application/`)

- **유스케이스** (`use_cases/`): 비즈니스 로직 구현
  - `PersonUseCases`: 사람 관리 작업 (생성, 삭제, 목록)
  - `AuthUseCases`: 인증 작업 (회원가입, 로그인, 세션 관리)
  - `QueryUseCases`: SurrealQL 원시 쿼리 실행
- **서비스** (`services/`): 유스케이스를 조율하는 애플리케이션 서비스
  - `CommandService`: 통합 인터페이스를 통한 모든 애플리케이션 명령 처리

### 3. 인프라 계층 (`src/infrastructure/`)

- **데이터베이스** (`database/`): 리포지토리 트레이트의 구체적 구현
  - `SurrealRepository`: 모든 리포지토리 인터페이스의 SurrealDB 구현

### 4. 표현 계층 (`src/presentation/`)

- **UI 컴포넌트** (`ui/components/`): 개별 탭 컴포넌트
  - `people_tab.rs`: 사람 관리 UI
  - `auth_tab.rs`: 인증 UI
  - `query_tab.rs`: 원시 쿼리 인터페이스
  - `session_tab.rs`: 세션 정보 표시
- **컨트롤러** (`controllers/`): UI 로직 및 상태 관리
  - `AppController`: 애플리케이션 상태 관리 및 유스케이스 조율
- **상태** (`state/`): UI 상태 관리
  - `AppState`: 애플리케이션 상태 구조체
- **앱** (`app.rs`): 메인 애플리케이션 UI 조율 및 eframe 통합

## 이 아키텍처의 장점

1. **관심사 분리**: 각 계층이 명확한 책임을 가짐
2. **테스트 용이성**: 비즈니스 로직이 격리되어 쉽게 테스트 가능
3. **유지보수성**: 한 계층의 변경이 다른 계층에 영향을 미치지 않음
4. **유연성**: 구현체 교체가 용이 (예: 다른 데이터베이스)
5. **의존성 역전**: 상위 모듈이 하위 모듈에 의존하지 않음

## 의존성 흐름

```
표현 계층 → 애플리케이션 계층 → 도메인 계층 ← 인프라 계층
```

- **표현 계층**: 애플리케이션 및 도메인에 의존
- **애플리케이션 계층**: 도메인에만 의존
- **인프라 계층**: 도메인에만 의존
- **도메인 계층**: 외부 의존성 없음 (순수 비즈니스 로직)

## 기술 구현

### 스레딩 아키텍처

멀티 스레드 아키텍처를 사용합니다:
- **메인 스레드**: eframe을 사용한 egui UI 실행
- **데이터베이스 스레드**: tokio 런타임으로 SurrealDB 작업을 비동기 처리
- **통신**: `std::sync::mpsc` 채널을 통한 스레드 간 명령/응답 통신

### 명령 패턴

모든 사용자 상호작용이 `Command` 열거형 변형으로 변환됩니다:
- `CreatePerson(String)`, `DeletePerson(String)`, `ListPeople`
- `SignUp`, `SignIn(String, String)`, `SignInRoot`, `Session`
- `RawQuery(String)`

### 메시지 시스템

UI에서 작업 결과를 메시지 시스템을 통해 표시합니다:
- `MessageType::Success` (녹색) 및 `MessageType::Error` (빨간색)
- 경과 시간을 표시하는 타임스탬프
- 메시지 이력 (최근 10개)

## 주요 의존성

`Cargo.toml` 기준:
- **egui 0.35.0 / eframe 0.35.0**: 즉시 모드 GUI 프레임워크 (한글 폰트 fallback 지원)
- **surrealdb 2.3.10**: 멀티 모델 데이터베이스 클라이언트
- **tokio 1.47.1**: 데이터베이스 작업용 비동기 런타임
- **serde 1.0.228**: 직렬화/역직렬화
- **anyhow 1.0.100**: 에러 처리
- **faker_rand 0.1.1 / rand 0.8.5**: 테스트 데이터 생성

## 실행 방법

1. SurrealDB 시작:
   ```bash
   surreal start --log trace --user root --pass root memory
   ```

2. 애플리케이션 실행:
   ```bash
   # 포맷 + 빌드 + 테스트
   ./scripts/run.sh

   # 또는 바로 실행
   ./scripts/run.sh run
   # 또는
   cargo run
   ```

3. 연결 테스트 (선택사항):
   ```bash
   cargo run --bin test_connection
   ```

## 기능

- **사람 관리**: 실시간 피드백이 있는 사람 생성, 삭제, 목록 조회
- **인증**: 회원가입, 로그인(사용자/루트), 세션 관리
- **원시 쿼리**: 결과 표시가 있는 커스텀 SurrealQL 쿼리 실행
- **세션 정보**: 현재 세션 상세 정보 및 인증 상태 조회
- **실시간 메시지**: 타임스탬프가 포함된 성공/오류 피드백
- **반응형 UI**: 로딩 상태가 있는 논블로킹 작업
