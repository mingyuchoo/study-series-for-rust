# Iced 주소록 앱

Iced GUI 프레임워크와 Clean Architecture를 적용한 주소록 데스크톱 애플리케이션입니다. SQLite를 사용하여 주소 데이터를 관리합니다.

## 주요 기능

- Clean Architecture 기반 계층 분리 (domain, application, infrastructure, presentation)
- SQLite 데이터베이스를 이용한 주소 데이터 영구 저장
- Iced 프레임워크를 활용한 GUI 인터페이스
- Tab / Shift+Tab 키로 입력 필드 간 포커스 이동 지원

## 프로젝트 구조

```
iced-app/
├── Cargo.toml          # Workspace 루트 설정
├── domain/             # 도메인 계층 (엔티티, 리포지토리 트레이트)
│   └── Cargo.toml      # 의존성: serde
├── application/        # 애플리케이션 계층 (유스케이스)
│   └── Cargo.toml      # 의존성: domain
├── infrastructure/     # 인프라 계층 (SQLite 구현체)
│   └── Cargo.toml      # 의존성: domain, rusqlite 0.32
├── presentation/       # 표현 계층 (Iced UI)
│   └── Cargo.toml      # 의존성: domain, application, infrastructure, iced 0.14.0
├── Makefile.toml       # 빌드 작업 자동화
└── rustfmt.toml        # Rust 포맷 설정
```

## 주요 의존성

- **iced 0.14.0**: Elm 아키텍처 기반 크로스 플랫폼 GUI 프레임워크 (tiny-skia 렌더러 사용)
- **rusqlite 0.32**: SQLite 데이터베이스 (bundled 피처)
- **serde 1.0**: 직렬화/역직렬화

> iced는 `default-features = false` + `tiny-skia, advanced, thread-pool` 피처 조합으로 사용합니다. Windows 환경에서 기본 wgpu(DX12) 백엔드가 일부 드라이버에서 `STATUS_ACCESS_VIOLATION`을 일으키는 이슈를 회피하기 위해 CPU 기반 `tiny-skia` 렌더러를 채택했으며, 0.14부터는 executor 피처(`thread-pool`)를 명시적으로 지정해야 합니다.

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

## 실행 방법

```shell
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

Iced 0.14는 Tab 포커스 순회를 자동으로 처리하지 않습니다. 각 `text_input`에 고유한 `id`를 부여하고, `keyboard::listen()` 구독에서 `Tab` 키를 감지해 `widget::operation::focus_next()` / `focus_previous()` Task를 디스패치해야 합니다. 또한 `iced::application(...)`에 `.subscription(...)`으로 구독을 등록해야 이벤트가 전달됩니다. 본 프로젝트의 `presentation/src/main.rs`가 이 패턴을 구현한 예시입니다.

### Windows: `STATUS_ACCESS_VIOLATION (0xc0000005)` 크래시

wgpu(DX12/Vulkan) 기본 백엔드가 일부 GPU 드라이버와 충돌하여 실행 직후 크래시가 발생하는 경우가 있습니다. 본 프로젝트는 `presentation/Cargo.toml`에서 iced 피처를 `tiny-skia`로 고정해 이 문제를 회피합니다. 다른 백엔드로 복귀하려면 `iced = "0.13.1"` 기본 설정으로 되돌린 뒤 환경변수 `WGPU_BACKEND=gl`로 우회할 수 있습니다.

## 참고 자료

- [Iced 예제 코드](https://redandgreen.co.uk/iced-rs-example-snippets/rust-programming/)
- [Iced 공식 문서](https://docs.rs/iced/)
