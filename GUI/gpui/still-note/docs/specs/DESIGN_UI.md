# DESIGN-UI: Stillnote DESIGN.md alignment

- Version: 1
- Status: READY
- Spec owner: orchestrator /root
- Base revision: 5d563f04a4b2391bacc21ee17beffea43fb58a84
- Request: 현재 UI/UX를 DESIGN.md에 충실히 맞춘다.

## 목표와 범위
현재 앱은 Rust/GPUI 네이티브 앱이며 웹 코드가 없다. 기존 저널의 기능과 데이터 계약을 유지하면서 DESIGN.md의 시각 언어를 실제 앱에 적용한다. 웹 플랫폼 전환, 마케팅/가격/사진 콘텐츠 추가는 범위 밖이다.

## 계약과 설계 제약
DESIGN.md의 핵심 컴포넌트 및 Do/Don't를 우선한다. 일부 illustrative example의 shadow/polarity 상충은 주 컴포넌트와 shadow-free 규칙을 따른다. Saans 자산이 없으면 라이선스 폰트 다운로드 없이 명시된 system sans fallback을 적용하고 문서화한다. GPUI 지원 범위 내에서 정확한 weight 652/456/300, line-height, 4/8px spacing을 사용한다. 기존 안정적인 element ID, 명령, 키보드/IME, 저장/읽기 전용 오류 처리를 유지한다. 색만으로 상태를 구분하지 않는다.

## Acceptance criteria
| AC-ID | Given / When | Then: 기대 동작 | 검증 방법 |
|---|---|---|---|
| AC-01 | 앱의 모든 로그/검색/오류 화면 | 흰 canvas, #141414 ink, #707070 muted, #adadad faint, #f0f0f0 field, #f3f3f3 soft surfaces, 지정 hairline. 청록/종이색/임의 accent/shadow 없음. blue는 상업 용도가 없어 사용하지 않음 | token unit + source/render inspection |
| AC-02 | 버튼/입력/카드/선택 UI | 버튼 및 navigation stadium pills, primary ink/white, secondary white hairline 또는 soft. 입력 rest borderless 16px corners, focused 2px ink ring. cards 24px hairline/soft, controls 최소 44px | GPUI E2E geometry/focus + source inspection |
| AC-03 | 모든 화면의 텍스트와 grouping | 명시한 fallback, heading 32/24px 652, body 16px 456, supporting 14px, metadata 12px, light lead 20px 300. 자연 tracking, 대문자 eyebrow 제거, 넉넉한 24/32/48px spacing. 앱 정보 architecture에 맞는 centered detached nav와 흑색 footer 적용 | token unit + render/source inspection |
| AC-04 | 600/768/1024/1360px 창 | 좁은 창에서 nav compact, 보조 panel 숨김/재배치, main 접근 가능. controls/입력/row actions 잘리지 않고 스크롤 가능. 창 min width는 600px 지원 | GPUI resize E2E + geometry assertions |
| AC-05 | 일간/월간/미래/컬렉션/인덱스/검색, 작성/수정/완료/이월/저장오류, Unicode input | 기존 사용자 동작과 데이터 보존, 명령/keyboard 회귀 없음 | 기존 unit/integration/E2E 전체 + 필요한 독립 추가 테스트 |

## 작업/ownership
Builder: src/ui.rs, src/input.rs, src/main.rs, src/lib.rs 및 필요한 src/theme.rs 신규, README.md. 공통 설정 변경 필요시 먼저 orchestrator에 보고. Test: tests/bujo_unit.rs, tests/bujo_integration.rs, tests/bujo_e2e.rs 및 .artifacts/design-ui 검증 증거. Reviewer: .artifacts/design-ui/review.md만. Orchestrator: 이 spec 및 상태/판정 보고서. DESIGN.md는 제공된 untracked 요구사항 문서로 변경하지 않고 checkpoint 포함. model/store/lockfile 기본 read-only.

## 검증
Git Bash C:/Program Files/Git/bin/bash.exe를 사용. 모든 writer 종료 후 builder 단독 bash scripts/format.sh --write 및 scoped checkpoint commit. Test는 clean revision에서 bash scripts/verify.sh를 순차 실행하며 5단계 결과, unit/integration/e2e 각각 실제 수집/실행 수, SHA, AC별 증거를 .artifacts/design-ui에 기록한다. 테스트 데이터는 임시 경로 사용. native GPU screenshot 관찰이 도구 제약으로 불가하면 명확히 기록하고 렌더 geometry 증거와 source inspection을 활용한다. reviewer는 동일 SHA에서 독립 검토. 필수 실패/미실행/미해결 리뷰는 FAIL.

## 가정/위험
필수 질문 없음. 사용자 웹 UI 표현은 이 저장소의 기존 앱 UI를 의미한다고 가정. 마케팅 예시를 저널 제품에 그대로 복제하지 않고 시각 토큰/관련 primitives를 적용. GPUI text shaping 및 OS fallback은 실제 Saans와 다를 수 있음.
