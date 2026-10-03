# DESIGN-02: Current DESIGN.md alignment

- Version: 1
- Status: READY
- Spec owner: /root
- Base revision: eacd6ff9d64e3704ce363fb91e2a9a49fbffadd6
- Request: 현재 코드베이스의 UI/UX를 DESIGN.md에 충실하게 수정.

## 목표와 범위
현재 Rust GPUI journal UI는 이전 monochrome white 디자인을 사용한다. 현재 DESIGN.md의 black/yellow 디자인 토큰과 컴포넌트 계약을 native UI에 적용한다. 새 웹 앱을 만들지 않으며 데이터 모델, 저장 형식과 기존 노트 기능은 보존한다. ClickHouse 마케팅 카피, SQL, 가격표 등 journal과 관계없는 내용은 추가하지 않는다.

## 계약과 설계 제약
DESIGN.md가 색상, 타이포그래피, 반경, 입력/버튼, 네비게이션의 권위 있는 기준이다. 앱의 탐색/작성 워크플로는 보존한다. 폰트는 가능하면 실제 Inter/JetBrains Mono 자산을 사용하며 fallback/환경 제한을 보고한다. 일반 본문에 yellow를 쓰지 않는다. 그림자, 새로운 hover 장식은 추가하지 않는다. focus border는 yellow이고 focus 전후 geometry는 같다. 최저 window width는 기존 600px를 유지하고 mobile은 600–767px이다. 원본 font/body/geometry 테스트의 이전 기대값은 DESIGN.md 변경에 맞추되 기능 assertion은 약화하지 않는다.

## Acceptance criteria

| AC-ID | Given / When | Then | 검증 방법 |
|---|---|---|---|
| AC-01 | 모든 journal view 렌더 | canvas #0a0a0a, ink #ffffff, body #cccccc, muted #888888, faint #5a5a5a, surface-soft #121212, card #1a1a1a, elevated #242424, borders #2a2a2a/#3a3a3a, primary #faff69, active #e6eb52, on-primary #0a0a0a 토큰 및 사용 | unit exact tokens + review render/input usages |
| AC-02 | 버튼/입력/card/focus | 일반 버튼/입력 40px 및 8px 반경, card 12px, primary CTA yellow/black, secondary dark/white, tabs transparent-muted 또는 card-white, focus yellow border geometry stable, pills badges only | unit geometry + e2e focus/reachability + review |
| AC-03 | heading/body/nav/stats | Inter heading 700, body 400, controls 600, nav 500; heading negative tracking; stats 56px/700 yellow; 본문 white/body gray; no shadows | unit type tokens + render review; font availability evidence |
| AC-04 | 600/768/1024/1360/1600px | top nav 64px black and mobile hamburger at <768 with reachable menu when opened; content centered <=1280px; narrow layout stacks and all controls reachable; desktop panels remain usable | e2e production render bounds/menu interaction |
| AC-05 | 일간/월간/미래/인덱스/컬렉션, create/edit/status/migrate/search/storage/IME | 기존 journal 동작과 Unicode 입력/저장 유지, 모든 action reachable; token design 적용은 동작을 깨지 않음 | existing unit/integration/e2e + targeted production interaction |

## 작업/병렬화 계획
Builder / code: src/**, assets/**, build.rs, Cargo.toml/Cargo.lock (필요시), README.md, .agents/verification.env/scripts/** (환경 수정 필요시 먼저 통보). Test / verifier: tests/**, .artifacts/design-02/**. Reviewer: .artifacts/design-02/** review reports only. Orchestrator: this spec and status/final reports. DESIGN.md는 읽기 전용이다. 같은 파일 동시 수정 금지. Test 작성 종료 후 builder만 전체 format하고 명시적으로 지정한 변경을 checkpoint commit한다. 새 revision이면 새 verify/review.

## 검증 환경과 명령
.agents/verification.env 실제 cargo 명령. bash scripts/format.sh --write 후 clean checkpoint에서 독립 verifier가 bash scripts/verify.sh 순차 실행. 각 unit/integration/e2e 로그에서 실행 수 >0, skip 수와 실패 확인. 임시 journal fixture 격리. GPUI headless production render 및 debug bounds로 layout 확인하며 가능한 시각 확인 증거를 제공한다. 실행 불가한 검증은 PASS로 기록하지 않는다.

## 질문과 가정
필수 질문 없음. 웹 UI라는 표현은 이 저장소의 사용자 인터페이스를 의미한다고 해석한다. 마케팅 섹션 96px 간격/hero 7-5/grid는 journal 전용 UI에 무조건 넣지 않고 작업 화면에 맞춰 4px spacing/24-32px card padding을 적용한다. Inter의 정확한 폰트/GPUI tracking 지원 제한은 evidence에 기록하고 조용히 대체하지 않는다.

## 완료 조건
모든 AC 증거, 동일 clean revision의 format/lint/unit/integration/e2e PASS(각 테스트 >0), 독립 reviewer PASS, Builder != Verifier를 만족해야 최종 PASS. 실패는 최대 3회 재작업 후 blocker 보고.
