# Dioxus App

데스크톱, 웹, 모바일을 지원하는 풀스택 Dioxus 애플리케이션입니다. 문서 관리, API 연동, 로컬 데이터베이스 저장 기능을 제공합니다.

## 주요 기능

- **멀티 플랫폼 지원**: 데스크톱(Linux/Windows/macOS), 웹(WASM), 모바일
- **문서 관리**: 로컬 SQLite 데이터베이스를 이용한 문서 CRUD 작업
- **API 연동**: JSONPlaceholder API를 통한 Posts, Todos, Users 데이터 연동
- **모던 UI**: 탭 인터페이스를 활용한 반응형 디자인
- **실시간 업데이트**: 라이브 데이터 동기화

## 사전 준비사항

### Ubuntu/Debian Linux

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev \
                 build-essential       \
                 pkg-config            \
                 libgtk-3-dev          \
                 libssl-dev            \
                 libsoup-3.0-dev       \
                 libxdo-dev
```

### Fedora/RHEL Linux

```bash
sudo dnf update
sudo dnf install glib2-devel         \
                 gtk3-devel          \
                 webkit2gtk4.1-devel \
                 libsoup3-devel      \
                 openssl-devel       \
                 pkg-config          \
                 libxdo-devel
```

### macOS

```bash
# Xcode Command Line Tools 설치
xcode-select --install

# Homebrew를 통한 의존성 설치
brew install pkg-config
```

### Windows

Visual Studio Build Tools 또는 Visual Studio Community를 C++ 개발 도구와 함께 설치하세요.

## 설치 방법

1. **Rust 설치** (미설치 시):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **Dioxus CLI 설치** (프로젝트 Dioxus 버전과 동일하게 맞출 것):
   ```bash
   cargo install cargo-binstall
   cargo binstall dioxus-cli@0.8.0-alpha.0
   ```

3. **프로젝트 클론 및 설정**:
   ```bash
   git clone <repository-url>
   cd dioxus-app
   ```

## 주요 의존성

`Cargo.toml` 기준 주요 의존성:

- **Dioxus 0.8.0-alpha.0**: 라우터·풀스택 기능을 지원하는 메인 프레임워크 (`router`, `fullstack` 피처; 플랫폼별 `web` / `desktop` / `mobile`)
- **Dioxus CLI (dx) 0.8.0-alpha.0**: 빌드·서빙·번들용 CLI (라이브러리와 동일 버전 권장)
- **Reqwest 0.13.4**: JSON 지원 HTTP 클라이언트 (API 연동용)
- **Rusqlite 0.40.1**: SQLite 데이터베이스 (`native-db` 피처 활성화 시, desktop/mobile에 포함)
- **Serde 1.0**: 직렬화/역직렬화 (`derive` 매크로 포함)
- **Serde JSON 1.0**: JSON 직렬화/역직렬화
- **Async-trait 0.1.89**: 비동기 트레이트 지원
- **Dirs 6.0**: 크로스 플랫폼 디렉터리 경로
- **Futures 0.3**: 비동기 유틸리티

**작성자**: mingyuchoo <mingyuchoo@gmail.com> (Rust Edition 2024)

## 프로젝트 구조

```
dioxus-app/
├── .cargo/
│   └── config.toml          # 링커 설정
├── assets/                  # 정적 자산 (CSS, 이미지 등)
├── src/
│   ├── application/         # 애플리케이션 계층 (서비스)
│   │   ├── doc_application_service.rs
│   │   ├── post_application_service.rs
│   │   ├── todo_application_service.rs
│   │   ├── user_application_service.rs
│   │   └── mod.rs
│   ├── domain/             # 도메인 계층 (엔티티, 리포지토리)
│   ├── infrastructure/     # 인프라 계층 (API, DB)
│   ├── presentation/       # 표현 계층 (UI 컴포넌트)
│   │   ├── docs.rs
│   │   ├── home.rs
│   │   ├── navbar.rs
│   │   ├── posts.rs
│   │   ├── todos.rs
│   │   ├── users.rs
│   │   └── mod.rs
│   └── main.rs            # 애플리케이션 진입점
├── Cargo.toml             # 의존성 및 피처 설정
├── Dioxus.toml            # Dioxus 설정
└── README.md
```

## 개발

### 애플리케이션 실행

**웹 (WASM) - 기본:**
```bash
dx serve
# 또는 명시적으로
dx serve --platform web
```

**데스크톱 (전체 기능 사용 시 권장):**
```bash
dx serve --platform desktop
```

**모바일 (추가 설정 필요):**
```bash
dx serve --platform mobile
```

### 프로덕션 빌드

**데스크톱:**
```bash
dx build --release --platform desktop
```

**웹:**
```bash
dx build --release --platform web
```

### 사용 가능한 피처

`Cargo.toml`에서 지원하는 피처 플래그:

- `desktop`: 네이티브 데이터베이스를 포함한 데스크톱 애플리케이션 (`native-db` 포함)
- `web`: 웹 애플리케이션 (WASM) - **기본 피처**
- `mobile`: 모바일 애플리케이션 (`native-db` 포함)
- `native-db`: `rusqlite`를 통한 SQLite 데이터베이스 지원 활성화

### 개발 명령어

- **재빌드**: 개발 서버에서 `r` 키 입력
- **자동 재빌드 전환**: `p` 키 입력
- **상세 로깅**: `v` 키 입력
- **종료**: `Ctrl+C`

### 웹 설정

`Dioxus.toml` 파일에서 웹 관련 설정을 구성합니다:
- **포트**: 8080 (기본 개발 서버 포트)
- **출력 디렉터리**: `dist/` (웹 빌드용)
- **공개 URL**: `/` (서브 디렉터리 배포 시 변경 가능)

## 애플리케이션 기능

### 문서 관리
- 문서 생성, 조회, 수정, 삭제
- 로컬 SQLite 데이터베이스 저장
- 아카이브/아카이브 해제 기능
- `~/.local/share/dioxus-app/`에 영구 데이터 저장

### API 연동
- **Posts**: JSONPlaceholder에서 블로그 게시물 조회 및 관리
- **Todos**: 완료 상태가 있는 할 일 관리
- **Users**: 사용자 정보 표시 및 관리

### 크로스 플랫폼 지원
- **데스크톱**: 전체 기능을 갖춘 네이티브 애플리케이션
- **웹**: API 기능이 포함된 브라우저 기반 애플리케이션
- **모바일**: 터치 최적화 인터페이스 (플랫폼별 추가 설정 필요)

## 문제 해결

### 링커 문제
Linux에서 링커 오류가 발생하면, 프로젝트에 포함된 `.cargo/config.toml` 파일이 x86_64-unknown-linux-gnu 타겟에 대해 `gcc`와 `bfd` 링커를 사용하도록 설정되어 있습니다.

### 의존성 누락
해당 플랫폼에 필요한 시스템 의존성이 모두 설치되어 있는지 확인하세요. 오류 메시지에서 누락된 라이브러리를 안내합니다.

### 데이터베이스 문제
SQLite 데이터베이스는 `~/.local/share/dioxus-app/docs.db`에 자동 생성됩니다. 데이터베이스 문제 발생 시 이 파일을 삭제하여 초기화할 수 있습니다.

## 기여

1. 리포지토리 포크
2. 기능 브랜치 생성
3. 변경사항 작성
4. 테스트 실행: `cargo test`
5. 포맷 확인: `cargo fmt`
6. Clippy 실행: `cargo clippy`
7. 풀 리퀘스트 제출

## 라이선스

[라이선스 정보 추가 필요]
