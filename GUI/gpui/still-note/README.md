# Stillnote · 나의 불렛저널

Rust와 **GPUI 0.2.2**로 만든 Windows 네이티브 개인 불렛저널입니다. 브라우저나 웹뷰 없이 GPU로 그리는 데스크톱 창을 사용합니다. DESIGN.md의 검정 canvas·노랑 CTA 디자인, 한국어 빠른 기록, 일간·월간·미래 로그, 컬렉션, 인덱스와 검색을 제공합니다. Ryder Carroll의 불렛저널 방법에서 영감을 받았으며 책 본문은 포함하지 않습니다.

## 실행

Windows 10/11 x64, 그래픽 드라이버, Rust stable, Visual Studio C++ Build Tools와 Windows SDK가 필요합니다. `rust-toolchain.toml`은 stable과 rustfmt/clippy를 지정합니다. 의존성은 Cargo.lock으로 고정되며 GPUI는 0.2.2입니다.

```powershell
cargo run --locked
```

빌드된 프로그램은 `target/debug/stillnote.exe`입니다. 처음 실행하면 빈 개인 저널이 열립니다. 기존 파일을 초기화하거나 샘플 기록을 자동으로 넣지 않습니다.

별도 파일로 안전하게 사용해 볼 수 있습니다:

```powershell
cargo run --locked -- --data-file .artifacts/my-test-journal.json
```

전체 개발 파이프라인은 **포맷 수정 → lint → 모든 테스트 대상 → doctest → production build → GUI 실행** 순서로 진행합니다. 실패하면 즉시 멈추고 해당 종료 코드를 반환합니다. 포맷 단계는 소스 파일을 수정하며, GUI는 창을 닫을 때까지 실행됩니다. PowerShell **7.4 이상**에서 실행하세요:

```powershell
pwsh -File scripts/run.ps1
pwsh -File scripts/run.ps1 --no-run
pwsh -File scripts/run.ps1 -- --data-file '.artifacts/my test journal.json'
```

Windows의 **Git Bash**에서도 같은 파이프라인을 실행할 수 있습니다. 다른 디렉터리에서 호출할 때는 스크립트 경로를 지정하면 됩니다. Windows 네이티브 GPUI를 빌드하므로 WSL/Linux 환경용 실행 명령은 아닙니다.

```bash
bash scripts/run.sh
bash scripts/run.sh --no-run
bash scripts/run.sh -- --data-file '.artifacts/my test journal.json'
```

`--no-run`은 모든 검사와 빌드를 완료하고 GUI 실행만 생략합니다. `--help`는 명령을 실행하지 않고 도움말을 출력합니다. `--` 뒤의 인수는 앱으로 그대로 전달됩니다. 기본 실행은 위의 개인 저널 경로를 사용하며, 테스트는 임시 저장 경로를 사용합니다. 전체 테스트는 `--all-targets --features test-support` 및 별도 doctest를 포함하고, 의존성을 해석하는 Cargo 명령에는 `--locked`를 사용합니다.

일반 실행은 `%LOCALAPPDATA%\Stillnote\Stillnote\data\journal.json`을 사용합니다. 앱은 네트워크/계정/클라우드를 사용하지 않습니다. 로컬 JSON은 암호화되어 있지 않습니다. Windows 계정과 디스크 보안을 통해 보호하세요.

## 릴리스 번들 및 설치 파일

Windows 네이티브 PowerShell **7.4 이상**, Rust stable의 rustfmt/clippy, Visual Studio C++ Build Tools와 Windows SDK, **Inno Setup 6.3 이상**이 필요합니다. Inno Setup은 PATH 또는 표준 설치 경로에서 찾으며, `ISCC_PATH` 환경변수로 `ISCC.exe` 경로를 지정할 수 있습니다.

```powershell
pwsh -File scripts/release.ps1
pwsh -File scripts/release.ps1 --target x86_64-pc-windows-msvc
pwsh -File scripts/release.ps1 --target aarch64-pc-windows-msvc
```

Git Bash는 같은 PowerShell 구현으로 인수를 전달합니다:

```bash
bash scripts/release.sh
bash scripts/release.sh --target aarch64-pc-windows-msvc
```

`--help`로 도움말을 확인합니다. 기본 아키텍처는 `rustc` host이며 x64와 ARM64 Windows MSVC를 지원합니다. 다른 아키텍처를 빌드하려면 `rustup target add TARGET`와 해당 MSVC 라이브러리가 필요합니다. 린트 및 테스트는 실행 가능한 host target에서 수행합니다. 포맷 수정 → strict Clippy → 전체 테스트 → doctest → release 빌드 → ZIP → 설치 EXE → SHA256 checksum 순서이며, 실패하면 해당 종료 코드로 멈춥니다. 앱이나 설치 파일은 자동 실행하지 않습니다.

산출물은 `.artifacts/releases/` 아래 매 실행마다 고유한 디렉터리에 생성됩니다. `stillnote-VERSION-ARCH.zip`, `stillnote-VERSION-ARCH-setup.exe`, `SHA256SUMS.txt`와 번들 원본을 포함합니다. Cargo metadata의 버전 및 실제 target 디렉터리를 사용하므로 `CARGO_TARGET_DIR`도 반영합니다. ZIP에는 실행 파일, 이 README, GPUI Apache 라이선스와 Pretendard SIL OFL 라이선스가 포함되며 폰트 데이터는 실행 파일에 내장됩니다. 개인 저널 파일은 포함하지 않습니다.

설치 파일은 **서명되지 않습니다**. 사용자가 직접 실행하면 현재 Windows 사용자 계정의 `%LOCALAPPDATA%\Programs\Stillnote`에 설치하고 시작 메뉴 바로가기와 제거 기능을 제공합니다. 제거해도 별도 개인 저널 데이터는 삭제하지 않습니다. ZIP과 설치 파일은 해당 아키텍처의 Windows와 그래픽 드라이버를 요구합니다. 실행 환경에는 아키텍처에 맞는 Microsoft Visual C++ 2015–2022 Redistributable을 설치하세요. VC runtime은 번들에 동봉하거나 자동으로 다운로드하지 않습니다.

## 사용

- **일간 로그**: 날짜를 입력하고 이동하거나 ‹ / › / 오늘을 누릅니다. 할 일, 이벤트, 메모를 선택하고 한 줄을 입력한 다음 Enter 또는 기록 +로 저장합니다.
- **할 일**: 완료/재개, 취소, 중요 표시와 수정이 가능합니다. 완료/취소한 일은 미완료 필터에 포함되지 않습니다. 이벤트·메모에는 완료/이월 버튼이 표시되지 않습니다.
- **월간 로그**: 달력 날짜를 누르면 해당 일간 로그로 이동합니다. 아래에는 해당 월의 월간 기록이 표시됩니다. ‹ / ›로 연말·연초를 포함해 다른 월을 봅니다.
- **미래 로그**: 월 버튼 또는 날짜 이동으로 월을 선택해 앞으로의 기록을 남깁니다.
- **컬렉션**: 컬렉션 패널에 이름을 입력하고 Enter 또는 만들기 버튼을 누릅니다. 선택한 컬렉션에 주제별 기록을 남깁니다.
- **인덱스**: 기록이 있는 날짜·월·컬렉션을 열 수 있습니다.
- **검색**: 오른쪽 위에 입력하면 모든 로그와 컬렉션을 검색합니다. 상태 버튼은 모든 기록 → 미완료 → 완료로 바뀝니다. 검색을 지우면 선택한 로그로 돌아옵니다. 검색 결과의 로그 열기로 원래 위치에 이동합니다.
- **이월**: 열린 할 일에서 이월 →를 누르고 일간/월간/미래와 대상 날짜를 선택합니다. 원본은 보존되어 `>` 또는 `<`로 표시되며 대상 열린 사본과 양방향으로 연결됩니다. 원본을 다시 이월하거나 수정할 수 없습니다. 원본/대상 링크를 누르면 이동합니다.

기호: `•` 할 일, `×` 완료, `○` 이벤트, `–` 메모, `>` 이월, `<` 미래 예약, `★` 중요, `⊘` 취소.

입력은 한국어 IME와 Unicode를 지원합니다. Enter 등록/수정, Ctrl+A 전체 선택, Ctrl+C/X/V 복사·잘라내기·붙여넣기, 방향키·Home·End·Shift+방향키 선택, Backspace/Delete 삭제를 사용할 수 있습니다. 클릭하면 커서와 2px 노랑 포커스 테두리가 표시됩니다. 창을 줄이면 보조 범례를 접고 기록 목록은 스크롤됩니다.

## 저장과 복구

각 변경은 버전이 있는 JSON으로 즉시 저장됩니다. 임시 파일을 같은 폴더에 기록하고 디스크 동기화한 다음 기존 파일을 `journal.json.bak`에 복사하고 교체합니다. 저장이 실패하면 메모리 상태도 확정하지 않으며 입력을 유지하고 오류를 보여 줍니다. 저장 직전 확인에서 다른 앱/창이 파일을 변경한 사실을 감지하면 충돌을 알리고 덮어쓰지 않습니다. 이때 앱을 재시작해 최신 데이터를 확인하세요. 현재 전체 쓰기 구간을 잠그지는 않으므로 같은 파일을 여러 앱 창에서 동시에 편집하지 마세요. 확인 직후의 동시 저장은 경합할 수 있습니다.

JSON 파싱, 버전, 날짜, ID, 컬렉션, 작업 상태 또는 이월 연결 검증이 실패하면 원본을 보존하고 읽기 전용 오류 상태로 시작합니다. 자동 초기화하지 않습니다.

백업 복구는 앱을 종료한 뒤 진행합니다:

1. 현재 `journal.json`을 별도 파일에 복사하여 보존합니다.
2. 검증 가능한 `journal.json.bak`을 `journal.json`으로 복사합니다.
3. 앱을 다시 실행해 내용을 확인합니다. 마지막 저장 이전 상태로 돌아갈 수 있습니다.

## 검증

저장소의 역할 계약에 따라 Builder와 독립 Test/Reviewer가 분리됩니다. 실제 명령은 `.agents/verification.env`와 PowerShell 설정에 있습니다.

```powershell
& 'C:/Program Files/Git/bin/bash.exe' -c 'bash scripts/format.sh --write'
& 'C:/Program Files/Git/bin/bash.exe' -c 'bash scripts/verify.sh'
```

format → clippy → 단위 → 파일 통합 → GPUI UI E2E를 순차 실행합니다. UI E2E는 `--features test-support`를 사용해 실제 view를 렌더하고 클릭·키보드 입력을 보냅니다. 임시 저장 경로를 사용하며 개인 데이터에는 접근하지 않습니다. 실제 Windows GPU 창 확인은 별도로 수행합니다.

입력 컴포넌트는 Zed Industries의 GPUI 0.2.2 공식 `examples/input.rs`를 바탕으로 구성하고 UTF-16 IME 위치 처리와 Windows 단축키를 보완했습니다. 원본 Apache-2.0 라이선스는 `assets/GPUI-LICENSE-APACHE`에 있습니다.


## 디자인

DESIGN.md의 검정 canvas (#0a0a0a), 흰 heading, 회색 본문, 노랑 primary CTA (#faff69), dark card (#1a1a1a)를 사용합니다. 버튼·입력은 40px 높이/8px 반경이고 카드는 12px 반경입니다. 포커스는 항상 확보된 2px 테두리의 색만 바뀌어 입력 크기를 유지합니다. 그림자나 추가 hover 장식은 없습니다.

사용자 지정에 따라 앱 내부 텍스트 전체는 Pretendard를 사용합니다. 공식 [Pretendard v1.3.9](https://github.com/orioncactus/pretendard/releases/tag/v1.3.9)의 full Korean OTF Regular(400), Medium(500), SemiBold(600), Bold(700)를 assets/fonts에 포함하고 GPUI에 등록합니다. 원본 SIL Open Font License 1.1은 assets/fonts/LICENSE-Pretendard.txt에 있으며 파일 출처와 SHA-256은 assets/fonts/SOURCES.md에 기록했습니다. 로컬 폰트 설치나 실행 중 네트워크 없이 heading/body/nav/button/input/placeholder/stat/help/error 모두 같은 family를 사용합니다. OS titlebar 및 미지원 emoji/symbol은 OS fallback을 허용합니다. DESIGN.md의 Inter/JetBrains 지정은 이번 사용자 요청으로 대체하고 크기·weight·색상·간격은 유지합니다. 로고·heading·stat·본문은 전체 문자열을 native shaping해 한글/결합문자와 글자 사이 kerning을 유지합니다. GPUI 0.2.2에 letter-spacing 속성이 없어 별도 음수 자간은 적용하지 않습니다. 글자 단위 flex wrapping은 제거하여 로고가 한 글자씩 세로로 쪼개지지 않습니다. 긴 기록·컬렉션·오류 문구는 native 줄바꿈하며 긴 단일 토큰도 폭에 맞춰 줄바꿈합니다. 입력은 caret-follow 가로 스크롤로 긴 문자열 끝까지 편집할 수 있습니다.

64px top nav는 항상 창 위에 고정되고 본문은 최대 1280px로 가운데 정렬됩니다. 최소 창 크기는 600×400px이며 768px 미만에서 메뉴 버튼으로 일간·월간·미래·인덱스를 엽니다. 항목을 선택하면 메뉴가 닫힙니다. 1024px 미만에서는 컬렉션을 스크롤 가능한 본문에 배치하며 보조 범례와 열린 할 일 stat은 너비 1180px·높이 650px부터 표시하며 해당 패널도 세로 스크롤됩니다. 높이 650px 미만에서는 컬렉션과 footer를 본문 스크롤에 넣어 모든 입력·동작에 접근할 수 있습니다. 마케팅 전용 SQL·가격표·96px 섹션 간격은 개인 저널 화면에 추가하지 않습니다.
