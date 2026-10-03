# window-app

Create GPUI App으로 생성된 윈도우 기반 gpui 애플리케이션입니다.

## 주요 의존성

- **gpui-pre 0.3.7** (`gpui` 이름으로 사용): GPU 가속 UI 프레임워크
- **gpui-pre-platform 0.3.7** (`gpui-platform` 이름으로 사용): OS별 앱 초기화 및 렌더링 백엔드
- **gpui-component 0.7.0**: gpui 컴포넌트 라이브러리
- **rust-embed 8.12.0**: SVG 아이콘 자산 로딩 및 임베딩
- **anyhow 1.0.104**: 에러 처리

`gpui-component 0.7.0`이 요구하는 `gpui-pre =0.3.7`과 동일한 버전을 사용하여
GPUI 타입 호환성을 유지합니다. 앱 초기화는 `gpui_platform::application()`을
사용합니다. `Cargo.lock`은 직접·간접 의존성의 검증된 버전을 기록하며 Git으로 관리합니다.

아이콘 경로는 `CARGO_MANIFEST_DIR` 기준이므로 프로젝트 루트에서 실행해도 자산을
찾을 수 있습니다. Windows에서는 한글 글꼴로 맑은 고딕(`Malgun Gothic`)을 사용합니다.

## 실행 방법

- Rust가 설치되어 있어야 합니다 - [Rustup](https://rustup.rs/)
- `rust-toolchain.toml`에 지정된 nightly 툴체인을 사용합니다.
- `cargo run --locked` 명령으로 실행

Windows 11에서는 Visual Studio 또는 Build Tools의 **Desktop development with C++**
워크로드(MSVC 컴파일러 및 Windows SDK)와 MSVC용 Rust 툴체인이 필요합니다.
Windows PowerShell 5.1 또는 PowerShell 7에서 다음 명령을 실행합니다.

```powershell
.\scripts\run.ps1
```

스크립트는 정리 → 빌드 → 테스트 → 실행을 순서대로 수행합니다. 어느 디렉터리에서
호출해도 프로젝트 루트에서 실행하며, 종료 시 원래 디렉터리로 복귀합니다.
Cargo 단계가 실패하면 즉시 중단하고 해당 종료 코드를 반환합니다.
추가 인자는 `cargo run`의 `--` 뒤로 전달됩니다.

```powershell
.\scripts\run.ps1 --example-argument "value with spaces"
```

스크립트 실행 정책으로 차단되는 경우 이번 실행에만 다음 명령을 사용할 수 있습니다.

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\run.ps1
```

macOS/Linux에서는 `bash scripts/run.sh`를 사용합니다. macOS의 Metal Toolchain
검사는 기존 스크립트에서 수행합니다.

## 릴리스 및 설치 프로그램 생성

현재 호스트의 OS/아키텍처를 대상으로 **포맷팅 → Clippy 린팅 → release 빌드 →
release 테스트 → 번들링 → 설치 프로그램 생성 → SHA-256 체크섬 생성**을 수행합니다.
Cargo 빌드·린팅·테스트에는 `--locked`를 사용하며 경고도 오류로 처리합니다.
버전과 Cargo 출력 경로는 `cargo metadata`에서 읽습니다. `CARGO_TARGET_DIR`도
지원하며, Cargo 설정의 기본 타깃과 혼동되지 않도록 호스트 타깃을 명시합니다.
버전은 현재처럼 `major.minor.patch` 형식이어야 합니다.

Windows:

```powershell
.\scripts\release.ps1
# CI에서는 소스 변경 없이 포맷을 검사합니다.
.\scripts\release.ps1 -CheckFormatting
# Inno Setup을 사용자 지정 경로에 설치했다면:
.\scripts\release.ps1 -IsccPath 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe'
```

Windows PowerShell 5.1 이상, x86/x64 MSVC Rust 툴체인, C++ Build Tools 및
[Inno Setup 6](https://jrsoftware.org/isinfo.php)이 필요합니다. 스크립트는 PATH와
Inno Setup 6의 표준 설치 경로를 검색합니다. ZIP에는 실행 파일, release 출력
디렉터리의 DLL, README가 포함됩니다. 설치 EXE는 사용자별
`%LOCALAPPDATA%\Programs\window-app`에 설치하며 시작 메뉴 바로가기와
제거 프로그램을 생성합니다. 배포 머신에 필요한 외부 런타임 DLL이 있다면
Cargo release 디렉터리에 준비해야 합니다.

macOS / Linux:

```sh
bash scripts/release.sh
bash scripts/release.sh --check-formatting
```

공통으로 Bash, Python 3, Rust nightly와 rustfmt/clippy가 필요합니다.

| OS | 추가 도구 / 조건 | 산출물 |
| --- | --- | --- |
| Windows | Inno Setup 6, MSVC (x86/x64) | ZIP, `-setup.exe` |
| macOS | Xcode, Metal Toolchain, `pkgbuild`, `hdiutil` (Intel/Apple Silicon) | `.app`, DMG, PKG |
| Debian/Ubuntu Linux | `dpkg-deb`, `dpkg-shlibdeps` (`dpkg-dev` 패키지), GPUI 빌드용 시스템 라이브러리 (amd64/arm64 GNU) | tar.gz, DEB |

macOS PKG는 `/Applications/window-app.app`에 설치합니다. DMG에는 앱과
Applications 폴더 바로가기가 포함됩니다. 시스템 외 dylib를 발견하면 누락된
의존성을 배포하지 않도록 중단하므로, 해당 의존성을 앱에 포함하고 경로를
수정하는 작업이 먼저 필요합니다. Metal Toolchain은
`xcodebuild -downloadComponent MetalToolchain`으로 설치할 수 있습니다.

Linux DEB는 `/usr/bin/window-app`과 애플리케이션 메뉴 항목을 설치합니다.
`dpkg-shlibdeps`로 빌드 머신의 라이브러리 패키지 정보에서 런타임 의존성을
산출합니다. 별도 설치 관리자 다운로드는 하지 않습니다.

```sh
sudo apt install dpkg-dev
# 생성된 DEB를 설치할 때:
sudo apt install ./dist/0.1.0/x86_64-unknown-linux-gnu/window-app-0.1.0-x86_64-unknown-linux-gnu.deb
```

산출물은 `dist/<버전>/<Rust 호스트 타깃>/`에 저장하며 Git에서 제외합니다.
`SHA256SUMS`에는 배포 파일의 체크섬이 기록됩니다. 실패하면 즉시 중단하고
0이 아닌 종료 코드를 반환합니다. 임시 패키징 폴더는 종료 시 삭제되며,
재실행하면 같은 버전/타깃의 산출물을 교체합니다. 실패한 실행의 일부 파일이
남을 수 있으므로 성공 종료와 체크섬 생성을 확인한 뒤 배포하세요.

SVG 아이콘은 실행 파일에 임베딩되므로 별도 assets 폴더를 배포하지 않습니다.
GPU 드라이버와 한글 글꼴은 대상 시스템에 필요합니다. Linux 바이너리와 DEB는
빌드한 배포판의 ABI/패키지 이름에 의존하므로 지원할 가장 오래된 배포판에서
빌드하고 실제 대상 환경에서 실행을 확인하세요. RPM/AppImage 및 크로스 컴파일은
지원하지 않습니다. 현재 설치 프로그램은 서명하지 않습니다. 공개 배포를 위한
Windows 코드 서명, macOS Developer ID 서명·공증은 별도 단계로 구성해야 합니다.

## 의존성 갱신 및 검증

직접 의존성은 루트 `Cargo.toml`의 `[workspace.dependencies]`에서 갱신합니다.
GPUI와 플랫폼 crate는 `gpui-component`이 요구하는 버전에 함께 맞춥니다.
간접 의존성은 각 crate가 허용하는 범위 내에서 최신 버전으로 갱신합니다.

```sh
cargo update
cargo fmt --all -- --check
cargo build --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## 참고 자료

- [`gpui`](https://www.gpui.rs/)
- [GPUI 문서](https://github.com/zed-industries/zed/tree/main/crates/gpui/docs)
- [GPUI 예제](https://github.com/zed-industries/zed/tree/main/crates/gpui/examples)
