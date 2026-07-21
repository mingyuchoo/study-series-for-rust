# Ratatui Diary

터미널 기반 다이어리 애플리케이션 (Rust + Ratatui)

## 기능

- 📅 월간 달력 뷰
- ✍️ Emacs 스타일 모드리스 에디터
- 💾 Markdown 파일 자동 저장
- 🎨 다이어리 작성 유무 시각적 표시
- 👁️ 실시간 Markdown 미리보기 (달력 & 에디터)
- 🎯 Selection 하이라이트
- 🔍 검색 매치 하이라이트
- ⌨️ 컨텍스트 기반 키바인딩 도움말

### 미리보기 기능

- **달력 화면**: 선택된 날짜의 다이어리 내용을 오른쪽에 실시간으로 표시
- **에디터 화면**: 작성 중인 Markdown 문서를 렌더링하여 오른쪽에 표시
- **화면 분할**: 50:50 레이아웃으로 원본과 미리보기를 동시에 확인
- **고급 Markdown 지원**: 헤더, 굵게, 기울임, 코드 블록, 리스트, 인용구, 표, 링크 등

## 설치

### 소스에서 설치

```bash
cargo build --release
cargo install --path crates/ratatui-diary
```

### Windows 설치 파일 만들기

`scripts/release.ps1`이 검증 → 릴리스 빌드 → Inno Setup 컴파일을 수행해
`dist/ratatui-diary-<버전>-<아키텍처>-setup.exe`를 만듭니다.

```powershell
# Inno Setup이 없으면 -InstallInno로 winget 자동 설치
./scripts/release.ps1 -InstallInno

# 검증 단계를 건너뛰고 빠르게 패키징만
./scripts/release.ps1 -SkipTest
```

생성된 설치 파일은 다음과 같이 동작합니다.

- `%LOCALAPPDATA%\Programs\Ratatui Diary`에 **사용자 단위**로 설치 (관리자 권한 불필요)
- 설치 마법사에서 **PATH 등록**과 **시작 메뉴 바로가기**를 선택 가능
- 제어판 '앱 및 기능'에서 제거하며, 제거 시 PATH 항목도 함께 정리

| 옵션 | 설명 |
|---|---|
| `-SkipTest` | fmt/clippy/test 검증 생략 |
| `-SkipBuild` | 기존 릴리스 바이너리를 그대로 패키징 |
| `-Version <ver>` | 설치 파일에 표기할 버전 (기본: Cargo.toml) |
| `-Target <triple>` | 크로스 빌드 대상 (기본: 호스트) |
| `-OutDir <path>` | 산출물 디렉터리 (기본: `dist/`) |
| `-InstallInno` | Inno Setup을 winget으로 자동 설치 |

## 사용법

```bash
ratatui-diary
```

### 달력 화면

| 키 | 동작 |
|---|---|
| `Ctrl+B` / `Ctrl+F` | 이전/다음 날짜 이동 |
| `Ctrl+P` / `Ctrl+N` | 이전/다음 주 이동 |
| `Alt+N` / `Alt+P` | 다음/이전 달 이동 |
| `Alt+]` / `Alt+[` | 다음/이전 연도 이동 |
| `Enter` | 다이어리 작성/편집 |
| `Ctrl+Q` | 종료 |

### 에디터 화면 (Emacs 스타일)

Emacs 스타일 모드리스 에디터로, 별도의 모드 전환 없이 항상 문자 입력이 가능합니다.
`Ctrl+`/`Alt+` 조합으로 명령을 실행합니다.

**커서 이동:**

| 키 | 동작 |
|---|---|
| `Ctrl+F` / `Ctrl+B` | 오른쪽/왼쪽 한 칸 이동 |
| `Ctrl+N` / `Ctrl+P` | 아래/위 한 줄 이동 |
| `Ctrl+A` / `Ctrl+E` | 줄 시작/줄 끝 이동 |
| `Alt+F` / `Alt+B` | 다음/이전 단어 이동 |
| `Alt+<` / `Alt+>` | 문서 시작/문서 끝 이동 |

**편집:**

| 키 | 동작 |
|---|---|
| 문자 입력 | 커서 위치에 삽입 (항상 가능) |
| `Enter` | 새 줄 |
| `Backspace` / `Ctrl+H` | 커서 앞 문자 삭제 |
| `Ctrl+D` | 커서 뒤 문자 삭제 |
| `Ctrl+O` | 커서 위치에 새 줄 열기 (open-line) |
| `Ctrl+K` | 커서~줄 끝 삭제 (kill-line) |

**Selection & 클립보드:**

| 키 | 동작 |
|---|---|
| `Ctrl+Space` | 마크 설정 (Selection 토글) |
| `Ctrl+W` | 선택 영역 잘라내기 |
| `Alt+W` | 선택 영역 복사 |
| `Ctrl+Y` | 붙여넣기 (yank) |

**Undo/Redo:**

| 키 | 동작 |
|---|---|
| `Ctrl+Z` | 실행취소 |
| `Ctrl+Shift+Z` | 다시실행 |

**검색 (Ctrl+S → 검색 서브모드):**

| 키 | 동작 |
|---|---|
| `Ctrl+S` | 검색 모드 진입 |
| 문자 입력 | 검색어 입력 |
| `Enter` | 검색 실행 |
| `Ctrl+S` | 다음 매치 |
| `Ctrl+R` | 이전 매치 |
| `Esc` | 검색 취소 |

**Ctrl+X 프리픽스 명령:**

| 키 | 동작 |
|---|---|
| `Ctrl+X` | 프리픽스 모드 진입 |
| `Ctrl+X → Ctrl+S` | 저장 |
| `Ctrl+X → Ctrl+C` | 달력으로 돌아가기 |
| `Esc` | 프리픽스 모드 취소 |

**기타:**

| 키 | 동작 |
|---|---|
| `Ctrl+Q` | 종료 |

**시각적 피드백:**
- Selection 영역: 회색 배경으로 하이라이트
- 검색 매치: 노란색 배경으로 표시
- 현재 매치: 밝은 노란색 + 굵게
- 서브모드 표시: 상태바에 `-- C-x --`, `검색: 패턴` 표시

## 데이터 저장

다이어리는 `~/.local/share/ratatui-diary/entries/` 디렉토리에 Markdown 파일로 저장됩니다.

파일명 형식: `YYYY-MM-DD.md`

## 유니코드 지원

- 한글(CJK) 문자의 전각(2칸) 너비를 올바르게 처리
- `unicode-width` 크레이트를 사용하여 커서 위치를 정확하게 계산
- 멀티바이트 문자(UTF-8)에 대한 바이트/문자 인덱스 변환 지원

## 아키텍처

ELM (Model-Update-View) 패턴 기반

- **Model**: 앱 상태
- **Update**: 상태 업데이트 로직
- **View**: UI 렌더링

## 개발

```bash
# 테스트 실행
cargo test

# 개발 모드 실행
cargo run
```

## 라이선스

MIT
