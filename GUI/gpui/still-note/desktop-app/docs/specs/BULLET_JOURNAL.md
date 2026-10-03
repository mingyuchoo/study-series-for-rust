# BUJO-01: 개인용 Rust GPUI 불렛저널

- Version: 1
- Status: READY
- Spec owner: orchestrator /root
- Base revision: initial scaffold (Git initialized for verification)
- Request: 현재 코드베이스 분석 후 Rust GPUI 개인용 불렛저널 GUI Desktop 앱 구현

## 목표와 범위

현재 저장소는 제품 코드 없이 역할 계약과 Bash/PowerShell 검증 스크립트만 포함한다. Windows에서 실행되는 네이티브 Rust GPUI 앱을 새로 구축한다. 한국어 UI와 Ryder Carroll의 빠른 기록, 인덱스, 일간/월간/미래 로그, 컬렉션, 수동 마이그레이션을 제공한다. 종이색 배경, 짙은 잉크 텍스트, 절제된 청록색 강조, 좌측 탐색과 중앙 기록지, 보조 범례/리뷰 영역을 사용한다. 계정/네트워크/클라우드 동기화/책 본문 복제는 범위 밖이다.

## 계약과 설계 제약

- Cargo package `stillnote`, 기본 `cargo run`은 실제 GPUI 창을 연다. GUI는 기본 기능이며, 핵심 모델은 GUI 없이 테스트 가능하다.
- `src/lib.rs`에서 모델/저장소 API를 export한다. Builder가 타입/메서드의 정확한 인터페이스를 Test agent와 확정한다.
- 모델: stable ID, 날짜, 종류(Task/Event/Note), 상태(Open/Complete/Cancelled/Migrated/Scheduled), 중요 표시, 텍스트, 선택적 collection, migration 원본/대상 연결. 일간/월간/미래 분류가 데이터로 명시된다.
- `•` 할 일, `×` 완료, `○` 이벤트, `–` 메모, `>` 이월, `<` 미래 예약. 완료/취소/이월된 작업은 열린 작업에 포함되지 않는다. 메모/이벤트에 할 일 완료/이월을 적용하지 않는다.
- 마이그레이션은 사용자가 대상 날짜/월을 선택하고 수행한다. 원본은 보존되어 > 또는 <로 표시되고 새 대상의 열린 항목과 연결된다. 이미 이월된 항목의 재이월을 거부한다. 날짜는 검증한다.
- 한국어/Unicode/IME 입력을 지원하는 입력 컴포넌트, 키보드 입력·삭제·붙여넣기·Enter 등록. 입력 포커스 표시와 읽을 수 있는 대비, 가변 창 크기, 기록 영역 스크롤.
- 로컬 사용자 데이터 디렉터리에 versioned JSON 저장, 테스트는 임시 디렉터리로 격리. 파일 손상/권한 실패를 UI에 표시하고 기존 파일을 덮어쓰지 않는다. 저장 실패 시 성공으로 표시하지 않는다. 안전한 교체 저장 및 기존 데이터 백업, 날짜/ID/상태/참조를 load 시 검증한다.
- 첫 실행은 빈 개인 저널과 안내. 데모는 명시적 옵션/별도 경로만 사용한다. 기존 데이터를 자동으로 seed/reset하지 않는다.
- 검색은 전체 로그/컬렉션에서 Unicode 텍스트를 찾으며 완료 상태 필터 및 날짜 이동을 제공한다. 수정/취소 동작으로 잘못 입력한 내용을 교정할 수 있다.
- 월간 로그는 월 캘린더와 월 작업을 표시하며 다른 월로 이동할 수 있다. 미래 로그는 미래 월별 기록을 추가/조회한다. 컬렉션 생성·선택·기록과 인덱스 탐색이 가능하다.
- 앱 실행 파일/데이터 경로/백업 복구/단축키/지원 환경을 README에 안내한다.

## Acceptance criteria

| AC-ID | Given / When | Then: 관찰 가능한 기대값 | 검증 방법 |
|---|---|---|---|
| AC-01 | Cargo build 및 정상 실행 | 웹 래퍼가 아닌 GPUI 네이티브 앱 창, 한국어 좌측 탐색/기록/범례, 빈 시작 안내 표시 | lint/build + GPUI render/input E2E 및 실제 창 관찰 |
| AC-02 | 일간 날짜에서 Task/Event/Note 한국어 항목 입력 | 종류/날짜/텍스트/ID가 정확히 저장되고 공백 입력은 거부됨 | unit + GUI input E2E |
| AC-03 | 열린 할 일 완료/재개/취소/중요 표시/텍스트 수정 | 올바른 기호/목록/개수가 갱신되고 Event/Note 완료 전이는 거부됨 | unit + E2E |
| AC-04 | 날짜 이동 및 월간/미래 탐색·등록 | 연말/윤년 포함 유효한 날짜/월, 각 로그에 맞는 기록만 보임 | unit + E2E |
| AC-05 | 컬렉션 생성/선택 및 인덱스 클릭 | 컬렉션 기록은 선택한 컬렉션에 보이고 인덱스로 해당 로그 이동 가능 | integration + E2E |
| AC-06 | 미완료 작업을 다른 일간/월간/미래 로그로 이월 | 원본 보존+>/ < 상태, 대상 열린 사본, 양방향 연결, 중복/부적절 이월 거부 | unit + integration + E2E |
| AC-07 | 저장 후 재시작 | 종류/상태/Unicode/컬렉션/이월 정보가 같은 값으로 복원됨 | 실제 파일 integration + 사용자 여정 E2E |
| AC-08 | 파일 손상 또는 저장 실패 | 오류 표시, 기존 파일 보존, 변경을 저장했다고 오인하지 않음 | integration 오류 사례 + GUI 오류 상태 E2E |
| AC-09 | 전체 검색/필터, 빈 결과 | 로그와 컬렉션에 걸친 결과 및 상태 필터가 정확하며 빈 결과 안내 | unit + E2E |
| AC-10 | 키보드/붙여넣기/한국어 입력 및 창 크기 변경 | 입력·등록/수정 사용 가능, 창 콘텐츠 읽기/스크롤 가능 | GPUI UI E2E + 실제 창 관찰 |

## 작업과 소유권

- Orchestrator /root: 이 spec, 배정 및 최종 보고서. 제품 코드/테스트 작성 금지.
- Builder: `src/**`, Cargo.toml/Cargo.lock, rust-toolchain.toml, `.cargo/**`, assets/**, README.md, .gitignore, `.agents/verification.env`, `.agents/verification.ps1`. 기존 scripts는 수정 필요 시 사전 보고. 모든 lock/config/build 산출물 단일 owner.
- Test agent: `tests/bujo_unit.rs`, `tests/bujo_integration.rs`, `tests/bujo_e2e.rs`, tests/fixtures/**, `.artifacts/**`. 기존 template tests 보존. 모든 writer 종료 이후 Builder가 전체 cargo fmt 실행.
- Reviewer: 제품/테스트/명령/spec 읽기 전용, `.artifacts/review/**` 보고서만 작성.

## 검증 환경과 명령

Windows x86_64 MSVC, 설치된 Rust stable/nightly 및 Git Bash 사용. Builder는 호환 GPUI release를 고정하고 MSVC/SDK 의존성을 진단한다. `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test --test bujo_unit`, `cargo test --test bujo_integration`, `cargo test --test bujo_e2e`를 실제 환경에 맞춰 verification.env에 지정한다. GPUI test-support 기능이 필요한 경우 명령에 포함하고 production GUI도 lint/build 범위에 포함한다. E2E는 GPUI 앱 view를 실행하고 입력/클릭/렌더를 통한 사용자 흐름을 검증한다. 도메인 함수만 호출하는 테스트를 GUI E2E라고 부르지 않는다. 실제 화면 관찰을 추가해 GPU 플랫폼과 레이아웃을 확인한다.

검증은 clean checkpoint에서 Test agent가 `bash scripts/verify.sh` 직접 실행. 각 범주 실제 테스트 최소 1개, skipped 0개. 로그와 AC별 결과는 `.artifacts/verification/`에 남긴다. Builder 진단은 독립 검증을 대체하지 않는다. Git 초기 scaffold checkpoint 및 Builder checkpoint commit은 이 계약에 따라 승인된 작업이다.

## 질문·가정·위험

필수 질문 없음. Windows 우선, 한국어 UI, 오프라인 개인 사용, JSON 파일 저장을 가정한다. GPUI API/Windows 컴파일과 실제 GPU 실행은 검증해야 하며 실패하면 원인과 한계를 보고한다. 개인 데이터에는 네트워크 접근을 사용하지 않는다. 앱 종료/재시작과 데이터 손상 시 기존 데이터를 보존한다.

## 완료 조건

모든 AC, 5단계 tool PASS, 같은 SHA의 독립 reviewer PASS, 역할 독립성 및 clean tree를 /root가 확인한다. 누락/실패를 숨기지 않으며 수정 후 새 SHA에서 전체 검증·리뷰한다.
