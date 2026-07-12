# 주소록 앱 - Clean Architecture with Tauri + Leptos

SQLite를 사용한 주소 데이터 CRUD 기능을 제공하는 데스크톱 애플리케이션입니다.
Clean Architecture 패턴을 적용하여 각 계층을 별도의 Rust 크레이트로 분리했습니다.

## 아키텍처

```
project-root/
├── domain/                 # 도메인 계층 (엔티티, 리포지토리 인터페이스)
├── application/            # 애플리케이션 계층 (유스케이스)
├── infrastructure/         # 인프라 계층 (SQLite 구현체)
├── presentation_backend/   # 백엔드 표현 계층 (Tauri 명령어)
├── presentation_frontend/  # 프론트엔드 표현 계층 (Leptos UI)
└── tauri-entrypoint/       # Tauri 애플리케이션 진입점
```

## 기능

- 주소 추가, 조회, 수정, 삭제 (CRUD)
- SQLite 데이터베이스 저장
- 반응형 웹 UI (Leptos)
- 데스크톱 애플리케이션 (Tauri)

## 개발 환경 설정

### 필수 요구사항

- Rust (최신 stable 버전)
- Tauri (데스크톱 애플리케이션 빌드용): `cargo install tauri-cli`
- Trunk (Leptos 빌드용): `cargo install trunk`
- SQLx CLI (데이터베이스 마이그레이션용): `cargo install sqlx-cli --no-default-features --features sqlite`

### 실행 방법

1. 의존성 설치:
```bash
cargo build
```

2. 개발 서버 실행:
```bash
cd tauri-entrypoint
cargo tauri dev
```

3. 프로덕션 빌드:
```bash
cd tauri-entrypoint
cargo tauri build
```

## 빌드 및 릴리즈

### 운영체제별 빌드

#### Windows
```bash
# Windows에서 실행
cd tauri-entrypoint
cargo tauri build --target x86_64-pc-windows-msvc
```

#### macOS
```bash
# macOS에서 실행
cd tauri-entrypoint
cargo tauri build --target x86_64-apple-darwin

# Apple Silicon (M1/M2) 지원
cargo tauri build --target aarch64-apple-darwin

# Universal Binary (Intel + Apple Silicon)
cargo tauri build --target universal-apple-darwin
```

#### Linux
```bash
# Linux에서 실행
cd tauri-entrypoint
cargo tauri build --target x86_64-unknown-linux-gnu

# AppImage 형태로 빌드
cargo tauri build --target x86_64-unknown-linux-gnu --bundles appimage

# RPM 패키지 빌드 (Fedora/RHEL/CentOS)
cargo tauri build --target x86_64-unknown-linux-gnu --bundles rpm

# DEB 패키지 빌드 (Ubuntu/Debian)
cargo tauri build --target x86_64-unknown-linux-gnu --bundles deb
```

### 크로스 컴파일 설정

다른 플랫폼용으로 빌드하려면 해당 타겟을 먼저 설치해야 합니다:

```bash
# Windows 타겟 추가
rustup target add x86_64-pc-windows-msvc

# macOS 타겟 추가
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Linux 타겟 추가
rustup target add x86_64-unknown-linux-gnu
```

### 릴리즈 파일 위치

빌드 완료 후 실행 파일은 다음 위치에 생성됩니다:

```
tauri-entrypoint/src-tauri/target/release/bundle/
├── msi/           # Windows Installer (.msi)
├── nsis/          # Windows NSIS Installer (.exe)
├── deb/           # Debian Package (.deb)
├── rpm/           # RPM Package (.rpm)
├── appimage/      # Linux AppImage
├── dmg/           # macOS Disk Image (.dmg)
└── macos/         # macOS App Bundle (.app)
```

### GitHub Actions를 통한 자동 릴리즈

`.github/workflows/release.yml` 파일을 생성하여 자동 빌드 및 릴리즈를 설정할 수 있습니다:

```yaml
name: Release
on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    permissions:
      contents: write
    strategy:
      fail-fast: false
      matrix:
        platform: [macos-latest, ubuntu-20.04, windows-latest]
    runs-on: ${{ matrix.platform }}

    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install dependencies (ubuntu only)
        if: matrix.platform == 'ubuntu-20.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y libgtk-3-dev libwebkit2gtk-4.0-dev libappindicator3-dev librsvg2-dev patchelf

      - name: Rust setup
        uses: dtolnay/rust-toolchain@stable

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: './tauri-entrypoint/src-tauri -> target'

      - name: Install frontend dependencies
        run: cargo install trunk

      - name: Build the app
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          projectPath: tauri-entrypoint
          tagName: ${{ github.ref_name }}
          releaseName: 'Address Book v__VERSION__'
          releaseBody: 'See the assets to download and install this version.'
          releaseDraft: true
          prerelease: false
```

### Fedora Linux 전용 RPM 패키지 빌드

#### 1. Fedora 빌드 환경 설정

```bash
# Fedora에서 필요한 개발 도구 설치
sudo dnf groupinstall "Development Tools" "C Development Tools and Libraries"
sudo dnf install webkit2gtk4.0-devel openssl-devel curl wget libappindicator-gtk3-devel librsvg2-devel

# Rust 설치 (rustup 사용 권장)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Trunk 설치
cargo install trunk
```

#### 2. RPM 패키지 빌드

```bash
# 1단계: 프론트엔드 빌드
cd presentation_frontend
trunk build

# 2단계: RPM 패키지 생성
cd ../tauri-entrypoint
cargo tauri build --bundles rpm

# 생성된 RPM 파일 위치 확인
ls -la target/release/bundle/rpm/
```

**중요**: `tauri.conf.json`에서 `beforeBuildCommand`가 올바르게 설정되지 않은 경우, 위와 같이 수동으로 프론트엔드를 먼저 빌드해야 합니다.

#### 3. RPM 패키지 설치

```bash
# 생성된 RPM 패키지 설치
sudo dnf install ./target/release/bundle/rpm/tauri-leptos-app-*.rpm

# 또는 rpm 명령어 사용
sudo rpm -ivh ./target/release/bundle/rpm/tauri-leptos-app-*.rpm
```

#### 4. Tauri 설정 커스터마이징 (선택사항)

`tauri-entrypoint/src-tauri/tauri.conf.json`에서 RPM 패키지 설정을 커스터마이징할 수 있습니다:

```json
{
  "tauri": {
    "bundle": {
      "linux": {
        "rpm": {
          "license": "MIT",
          "depends": ["webkit2gtk4.0"],
          "files": {
            "/usr/share/applications/": "assets/addressbook.desktop",
            "/usr/share/icons/hicolor/256x256/apps/": "icons/256x256.png"
          }
        }
      }
    }
  }
}
```

#### 5. 데스크톱 엔트리 파일 생성

`tauri-entrypoint/src-tauri/assets/addressbook.desktop` 파일을 생성:

```desktop
[Desktop Entry]
Version=1.0
Type=Application
Name=주소록 앱
Comment=SQLite 기반 주소록 관리 애플리케이션
Exec=addressbook-app
Icon=addressbook-app
Terminal=false
Categories=Office;Database;
StartupWMClass=addressbook-app
```

#### 6. 패키지 검증

```bash
# RPM 패키지 정보 확인
rpm -qip ./target/release/bundle/rpm/tauri-leptos-app-*.rpm

# 패키지 내용 확인
rpm -qlp ./target/release/bundle/rpm/tauri-leptos-app-*.rpm

# 설치된 패키지 확인
rpm -qa | grep tauri-leptos-app
```

#### 7. 자동화된 Fedora 빌드 (GitHub Actions)

`.github/workflows/fedora-build.yml`:

```yaml
name: Fedora RPM Build
on:
  push:
    tags:
      - 'v*'

jobs:
  build-fedora:
    runs-on: ubuntu-latest
    container:
      image: fedora:latest

    steps:
      - name: Install dependencies
        run: |
          dnf update -y
          dnf groupinstall -y "Development Tools" "C Development Tools and Libraries"
          dnf install -y webkit2gtk4.0-devel openssl-devel curl wget libappindicator-gtk3-devel librsvg2-devel git

      - name: Install Rust
        run: |
          curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
          source ~/.cargo/env
          echo "$HOME/.cargo/bin" >> $GITHUB_PATH

      - name: Checkout code
        uses: actions/checkout@v4

      - name: Install Trunk
        run: |
          source ~/.cargo/env
          cargo install trunk

      - name: Build RPM
        run: |
          source ~/.cargo/env
          cd tauri-entrypoint
          cargo tauri build --bundles rpm

      - name: Upload RPM artifact
        uses: actions/upload-artifact@v3
        with:
          name: fedora-rpm
          path: tauri-entrypoint/src-tauri/target/release/bundle/rpm/*.rpm
```

### 릴리즈 노트

각 릴리즈에는 다음 정보를 포함하는 것을 권장합니다:

- 새로운 기능
- 버그 수정
- 성능 개선
- 호환성 변경사항
- 설치 방법 및 시스템 요구사항

### 프로젝트 구조

- **domain**: 비즈니스 로직의 핵심 엔티티와 리포지토리 인터페이스 정의
- **application**: 유스케이스 구현 (비즈니스 로직 조율)
- **infrastructure**: SQLite 데이터베이스 구현체 (SQLx 사용)
- **presentation_backend**: Tauri 명령어 핸들러 및 상태 관리
- **presentation_frontend**: Leptos 기반 웹 UI (CSR 모드)
- **tauri-entrypoint**: Tauri 애플리케이션 실행 진입점

## Clean Architecture 설계

이 프로젝트는 Clean Architecture 원칙을 따라 의존성이 외부에서 내부로만 향하도록 설계되었습니다.

### 계층별 역할 및 의존성

```
┌─────────────────────────────────────────────────────────┐
│  Presentation Layer (표현 계층)                         │
│  ┌──────────────────────┐  ┌──────────────────────┐    │
│  │ presentation_frontend│  │ presentation_backend │    │
│  │  - Leptos UI         │  │  - Tauri Commands    │    │
│  │  - 사용자 인터페이스 │  │  - API 핸들러        │    │
│  └──────────────────────┘  └──────────────────────┘    │
└─────────────────────────────────────────────────────────┘
                    ↓                    ↓
┌─────────────────────────────────────────────────────────┐
│  Application Layer (애플리케이션 계층)                  │
│  ┌──────────────────────────────────────────────────┐   │
│  │ application                                      │   │
│  │  - AddressService (유스케이스)                   │   │
│  │  - 비즈니스 로직 조율                            │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│  Domain Layer (도메인 계층) - 핵심 비즈니스 로직        │
│  ┌──────────────────────────────────────────────────┐   │
│  │ domain                                            │  │
│  │  - Address Entity (엔티티)                        │  │
│  │  - AddressRepository Trait (인터페이스)           │  │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
                           ↑
┌─────────────────────────────────────────────────────────┐
│  Infrastructure Layer (인프라 계층)                     │
│  ┌──────────────────────────────────────────────────┐   │
│  │ infrastructure                                    │  │
│  │  - SqliteAddressRepository (구현체)               │  │
│  │  - 데이터베이스 연결 및 쿼리                      │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### 각 계층의 상세 구조

#### 1. Domain Layer (도메인 계층)
가장 안쪽 계층으로, 외부 의존성이 전혀 없습니다.

```
domain/
├── entities/
│   └── address.rs             # Address 엔티티 정의
└── repositories/
    └── address_repository.rs  # AddressRepository trait 정의
```

- **엔티티**: 비즈니스 규칙을 담은 핵심 객체 (Address)
- **리포지토리 인터페이스**: 데이터 접근 추상화 (trait)

#### 2. Application Layer (애플리케이션 계층)
도메인 계층만 의존합니다.

```
application/
└── usecases/
    └── address_service.rs  # AddressService 구현
```

- **유스케이스**: 비즈니스 로직 조율 (CRUD 작업)
- 리포지토리 인터페이스를 통해 데이터 접근

#### 3. Infrastructure Layer (인프라 계층)
도메인 계층의 인터페이스를 구현합니다.

```
infrastructure/
└── database/
    └── sqlite_repository.rs  # SqliteAddressRepository 구현
```

- **구현체**: AddressRepository trait의 SQLite 구현
- SQLx를 사용한 실제 데이터베이스 작업

#### 4. Presentation Layer (표현 계층)
애플리케이션 계층을 사용하여 사용자와 상호작용합니다.

**Backend (presentation_backend)**
```
presentation_backend/
├── models/
│   └── dto.rs              # 데이터 전송 객체
└── commands/
    └── address_commands.rs # Tauri 명령어 핸들러
```

**Frontend (presentation_frontend)**
```
presentation_frontend/
└── src/
    ├── app.rs              # 메인 앱 컴포넌트
    ├── components/         # UI 컴포넌트
    └── api/                # Tauri API 호출
```

### Clean Architecture의 이점

1. **독립성**: 각 계층이 독립적으로 테스트 가능
2. **유연성**: 데이터베이스나 UI 프레임워크 교체 용이
3. **유지보수성**: 비즈니스 로직이 외부 기술에 종속되지 않음
4. **확장성**: 새로운 기능 추가 시 기존 코드 영향 최소화

## 권장 IDE 설정

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
