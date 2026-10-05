# Iced 주소록 앱

Iced GUI 프레임워크와 Clean Architecture를 적용한 주소록 데스크톱 애플리케이션입니다. SQLite를 사용하여 주소 데이터를 관리합니다.

## 주요 기능

- Clean Architecture 기반 계층 분리 (domain, application, infrastructure, presentation)
- SQLite 데이터베이스를 이용한 주소 데이터 영구 저장
- Iced 프레임워크를 활용한 GUI 인터페이스
- Tab / Shift+Tab 키로 입력 필드 간 포커스 이동 지원
- 상단 알약형 토글 버튼으로 한국어 / English 즉시 전환 (입력 내용 유지)
- 시스템 / 라이트 / 다크 테마 전환 및 시스템 모드에서 OS 테마 변경 자동 반영
- 한국어 폰트 내장 및 입력 검증 오류 다국어 표시

기본 언어는 영어, 기본 테마는 시스템입니다. 상단 `언어(Language)`와 `테마(Theme)` 버튼 그룹에서 선택할 수 있으며, 활성 버튼은 DESIGN.md의 category-tab-active 스타일로 표시됩니다. 설정은 현재 실행 중에 유지되며 재실행 시 기본값으로 돌아갑니다.

## 프로젝트 구조

```
iced-app/
├── Cargo.toml          # Workspace 루트 설정
├── Cargo.lock          # 직접/간접 의존성 잠금 파일
├── crates/             # Rust crate 모음
│   ├── domain/         # 도메인 계층 (엔티티, 리포지토리 트레이트)
│   │   └── Cargo.toml  # 의존성: serde, thiserror
│   ├── application/    # 애플리케이션 계층 (유스케이스)
│   │   └── Cargo.toml  # 의존성: domain, thiserror
│   ├── infrastructure/ # 인프라 계층 (SQLite 구현체)
│   │   └── Cargo.toml  # 의존성: domain, rusqlite 0.40.2
│   └── presentation/   # 표현 계층 (Iced UI, fonts/ 포함)
│       └── Cargo.toml  # 의존성: domain, application, infrastructure, iced 0.14.0
├── Makefile.toml       # 빌드 작업 자동화
└── rustfmt.toml        # Rust 포맷 설정
```

## 주요 의존성

- **iced 0.14.0**: Elm 아키텍처 기반 크로스 플랫폼 GUI 프레임워크 (tiny-skia 렌더러 사용)
- **rusqlite 0.40.2**: SQLite 데이터베이스 (bundled 피처)
- **serde 1.0.229**: 직렬화/역직렬화
- **thiserror 2.0.21**: 도메인/애플리케이션 오류 타입 정의

> iced는 `default-features = false` + `tiny-skia, advanced, thread-pool, x11, wayland` 피처 조합으로 사용합니다. Windows 환경에서 기본 wgpu(DX12) 백엔드가 일부 드라이버에서 `STATUS_ACCESS_VIOLATION`을 일으키는 이슈를 회피하기 위해 CPU 기반 `tiny-skia` 렌더러를 채택했으며, 0.14부터는 executor 피처(`thread-pool`)와 Linux 윈도우 시스템 피처(`x11`, `wayland`)를 명시적으로 지정해야 합니다.

## 사전 준비사항

- Rust (최신 stable 버전 권장)
- Cargo (Rust와 함께 설치됨)

## 설치 방법

1. 프로젝트 클론 또는 생성:
```shell
git clone <repository-url>
cd iced-app
```

2. 의존성은 Cargo가 자동으로 처리합니다.

`Cargo.lock`을 함께 관리하여 직접/간접 의존성 버전을 재현합니다. 버전 갱신 후에는 `cargo update`로 잠금 파일을 갱신하고, `cargo test --workspace --locked`와 `cargo clippy --workspace --all-targets --locked -- -D warnings`로 검증합니다.

## 실행 방법

workspace 루트(`Cargo.toml`이 있는 디렉터리)에서 빌드, 테스트 및 실행합니다. 빌드 결과는 루트의 `target/`에 생성되고 주소록 데이터는 루트의 `addresses.db`에 저장됩니다.

```shell
cargo build --workspace --locked
cargo test --workspace --locked
cargo run -p presentation
```

## 문제 해결

### 링커 오류: "invalid linker name in argument '-fuse-ld=mold'"

컴파일 중 이 오류가 발생하면 `mold` 링커가 설정되어 있지만 설치되지 않은 것입니다. 다음과 같이 설치하세요:

**Fedora/RHEL:**
```shell
sudo dnf install mold
```

**Ubuntu/Debian:**
```shell
sudo apt install mold
```

**Arch Linux:**
```shell
sudo pacman -S mold
```

또는 Rust 설정 파일에서 링커 설정을 제거하여 mold를 비활성화할 수 있습니다.

### Tab 키로 입력 필드 간 이동이 되지 않을 때

Iced 0.14는 Tab 포커스 순회를 자동으로 처리하지 않습니다. 각 `text_input`에 고유한 `id`를 부여하고, `keyboard::listen()` 구독에서 `Tab` 키를 감지해 `widget::operation::focus_next()` / `focus_previous()` Task를 디스패치해야 합니다. 또한 `iced::application(...)`에 `.subscription(...)`으로 구독을 등록해야 이벤트가 전달됩니다. 본 프로젝트의 `crates/presentation/src/main.rs`가 이 패턴을 구현한 예시입니다.

### Windows: `STATUS_ACCESS_VIOLATION (0xc0000005)` 크래시

wgpu(DX12/Vulkan) 기본 백엔드가 일부 GPU 드라이버와 충돌하여 실행 직후 크래시가 발생하는 경우가 있습니다. 본 프로젝트는 `crates/presentation/Cargo.toml`에서 iced 피처를 `tiny-skia`로 고정해 이 문제를 회피합니다. GPU 백엔드를 쓰려면 `wgpu` 피처를 추가한 뒤 환경변수 `WGPU_BACKEND=gl` 등으로 우회할 수 있습니다.

## 참고 자료

- [Iced 예제 코드](https://redandgreen.co.uk/iced-rs-example-snippets/rust-programming/)
- [Iced 공식 문서](https://docs.rs/iced/)
