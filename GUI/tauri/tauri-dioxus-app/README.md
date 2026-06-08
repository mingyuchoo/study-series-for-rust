# 주소록 앱 (Address Book App)

SQLite를 사용하는 Clean Architecture 기반의 크로스 플랫폼 주소록 데스크톱 애플리케이션입니다. Tauri와 Dioxus를 사용하여 프론트엔드와 백엔드 모두 Rust로 개발되었습니다.

## 프로젝트 구조

```
tauri-dioxus-app/
├── Cargo.toml                 # Workspace 루트
├── domain/                    # 도메인 계층 (엔티티, 리포지토리 인터페이스)
├── application/               # 애플리케이션 계층 (유스케이스)
├── infrastructure/            # 인프라 계층 (SQLite 구현체)
├── presentation_backend/      # 백엔드 표현 계층 (Tauri 명령어)
│   ├── src/
│   ├── Cargo.toml
│   ├── tauri.conf.json        # Tauri 설정
│   └── capabilities/
├── presentation_frontend/     # 프론트엔드 표현 계층 (Dioxus UI)
│   ├── src/
│   ├── Cargo.toml
│   ├── Dioxus.toml            # Dioxus 설정
│   └── assets/
├── Makefile.toml              # 빌드 작업 자동화
└── rustfmt.toml               # Rust 포맷 설정
```

## 사전 준비사항

- [Rust](https://rustup.rs/) (최신 stable 버전)
- [Tauri CLI](https://tauri.app/start/prerequisites/): `cargo install tauri-cli`
- [Dioxus CLI](https://dioxuslabs.com/learn/0.6/getting_started): `cargo install dioxus-cli`

## 주요 의존성

`Cargo.toml` (Workspace) 기준:

- **Tauri 2**: 데스크톱 애플리케이션 프레임워크
- **Dioxus 0.7**: 프론트엔드 UI 프레임워크 (웹 피처)
- **SQLx 0.9**: SQLite 데이터베이스 (runtime-tokio, sqlite, chrono, uuid 피처)
- **Tokio 1**: 비동기 런타임
- **Serde 1 / Serde JSON 1**: 직렬화/역직렬화
- **Chrono 0.4**: 날짜/시간 처리
- **UUID 1**: 고유 식별자 생성
- **wasm-bindgen 0.2 / web-sys 0.3**: WebAssembly 바인딩

## 개발

### 빠른 시작

```shell
# 프로젝트 디렉터리로 이동
cd tauri-dioxus-app

# 개발 서버 실행
cargo tauri dev
```

### 사용 가능한 명령어

cargo-make 사용 시 (`cargo install cargo-make`):

```shell
# 개발
cargo make run              # 개발 서버 시작
cargo make watch-run        # 파일 감시 모드로 시작

# 코드 품질
cargo make check            # 코드 검사
cargo make clippy           # 린터 실행
cargo make format           # 코드 포맷
cargo make test             # 테스트 실행

# 빌드
cargo make build            # 개발 빌드
cargo make release          # 프로덕션 빌드
cargo make clean            # 빌드 결과물 정리
```

### 직접 명령어

```shell
# 개발
cargo tauri dev             # 개발 서버 시작
dx serve --port 1420        # Dioxus 개발 서버만 시작

# 빌드
cargo tauri build           # 프로덕션 빌드
dx bundle --release         # Dioxus 번들만 빌드
```

## 기능

- **연락처 관리**: 이름, 이메일, 전화번호, 주소 정보 저장
- **검색 기능**: 모든 필드에서 연락처 검색 가능
- **CRUD 작업**: 연락처 생성, 조회, 수정, 삭제
- **SQLite 데이터베이스**: 로컬 데이터 저장
- **Clean Architecture**: 도메인, 애플리케이션, 인프라, 표현 계층 분리
- **크로스 플랫폼**: Windows, macOS, Linux에서 실행
- **Rust 풀스택**: 프론트엔드(Dioxus)와 백엔드(Tauri) 모두 Rust 사용
- **타입 안전성**: 전체 스택에서 Rust 타입 안전성 보장

## 사용법

### 연락처 추가
1. "새 연락처" 버튼 클릭
2. 이름 (필수), 이메일, 전화번호, 주소 입력
3. "추가" 버튼 클릭

### 연락처 검색
1. 상단 검색창에 검색어 입력
2. "검색" 버튼 클릭 또는 Enter 키 입력
3. 이름, 이메일, 전화번호, 주소에서 검색됩니다

### 연락처 수정
1. 연락처 카드에서 "수정" 버튼 클릭
2. 정보 수정 후 "수정" 버튼 클릭

### 연락처 삭제
1. 연락처 카드에서 "삭제" 버튼 클릭

## 설정

- **Dioxus 설정**: `presentation_frontend/Dioxus.toml` - 프론트엔드 빌드 설정
- **Tauri 설정**: `presentation_backend/tauri.conf.json` - 앱 메타데이터 및 빌드 설정
- **빌드 작업**: `Makefile.toml` - 개발 워크플로우 자동화

## 아키텍처

이 애플리케이션은 Clean Architecture 패턴을 따릅니다:

```
┌─────────────────────────────────────────────────────────┐
│  표현 계층 (Presentation Layer)                         │
│  ┌──────────────────────┐  ┌──────────────────────┐    │
│  │ presentation_frontend│  │ presentation_backend │    │
│  │  - Dioxus UI         │  │  - Tauri Commands    │    │
│  └──────────────────────┘  └──────────────────────┘    │
└────────────────────┬───────────────────┬────────────────┘
                     ↓                   ↓
┌─────────────────────────────────────────────────────────┐
│  애플리케이션 계층 (Application Layer)                  │
│  - 유스케이스 및 비즈니스 로직 조율                     │
└────────────────────────────┬────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────┐
│  도메인 계층 (Domain Layer) - 핵심 비즈니스 로직        │
│  - 엔티티, 리포지토리 트레이트                          │
└────────────────────────────┬────────────────────────────┘
                             ↑
┌─────────────────────────────────────────────────────────┐
│  인프라 계층 (Infrastructure Layer)                     │
│  - SQLite 데이터베이스 구현체 (SQLx)                    │
└─────────────────────────────────────────────────────────┘
```

## 권장 IDE 설정

[VS Code](https://code.visualstudio.com/) + 확장 프로그램:
- [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
- [Dioxus](https://marketplace.visualstudio.com/items?itemName=DioxusLabs.dioxus)

## 참고 자료

- [Tauri 공식 문서](https://tauri.app/)
- [Dioxus 공식 문서](https://dioxuslabs.com/)
- [Rust Book](https://doc.rust-lang.org/book/)
