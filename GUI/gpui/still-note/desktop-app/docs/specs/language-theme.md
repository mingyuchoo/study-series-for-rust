# LANGUAGE-THEME: 언어와 테마 선택

- Version: 2
- Status: READY
- Spec owner: /root/spec
- Base revision: 9c0cf9f0f58b6c89450d7263802b0a976ad840e5
- Request: 한국어/영어 및 시스템/라이트/다크 선택 버튼, 즉시 반영과 설정 저장에 대한 사용자 승인.

## 목표와 범위

현재 UI와 입력창은 한국어 문구 및 고정 다크 색상을 사용한다. 두 언어와 세 테마 모드를 직접 선택하고 재실행 시 복원한다. 기록, 컬렉션 이름, 검색어 등 사용자 콘텐츠는 그대로 유지한다. 앱 문구, placeholder, 날짜/위치 설명, 창 제목, 상태 안내, 사용자에게 표시하는 검증·저장·복구 오류를 포함한다. OS 자체 오류의 원문 상세, 브랜드 stillnote, ISO 날짜 입력 형식은 번역 대상에서 제외한다. 번역 라이브러리, 추가 언어, 저널 스키마 변경은 범위 밖이다.

## 계약과 설계 제약

- `Language { Korean, English }`, `ThemeMode { System, Light, Dark }`와 작은 serde 설정 타입을 사용한다. 기본값은 Korean/System이다. 이름과 공개 인터페이스는 builder/test가 확정하되 동일한 관찰 가능 계약을 제공한다.
- 설정은 저널 파일과 같은 디렉터리의 `settings.json`으로 분리한다. `--data-file`을 다른 디렉터리로 지정하면 설정도 격리된다. 같은 디렉터리의 여러 저널은 설정을 공유한다.
- 설정 누락은 정상 기본값이다. 잘못된 JSON/enum 또는 읽기 실패는 기본값과 현지화된 안내로 처리하고 저널 접근을 막지 않는다. 잘못된 파일을 자동으로 덮어쓰지 않는다. 명시적 설정 변경으로도 원본 오류 파일을 조용히 파괴하지 않으며, 안전한 원본 보존 또는 저장 거부와 안내 중 작은 구현을 선택한다.
- 쓰기 실패 시 앱 내 선택은 유지하고 저장 실패를 안내한다. 저널 파일·모델은 변경하지 않는다. 설정 파일 쓰기도 부분 파일로 기존 정상 설정을 잃지 않도록 기존 atomic-write 패턴을 재사용한다.
- 입력 Entity를 재생성하거나 `set_text`로 placeholder를 바꾸지 않는다. draft/selection/cursor/IME 상태, 검색어, 날짜, 로그/필터, 편집·이동 상태와 사용자 데이터를 보존한다. 선택 버튼을 조작하기 위해 포커스 이동은 허용한다.
- 언어 버튼은 `[한국어 | English]`, 테마 버튼은 번역된 `[시스템 | 라이트 | 다크]` / `[System | Light | Dark]`로 직접 선택한다. 선택 상태와 키보드 포커스가 시각적으로 명확하며 키보드만으로 이동·활성화할 수 있다.
- 넓은 창에서는 상단 오른쪽, 공간이 부족하면 기존 메뉴에 배치한다. 메뉴 전환 폭은 영어 네비게이션과 설정 및 3개 창 제어 버튼이 실제로 들어가는 폭에 맞춰 조정할 수 있다. 600px 최소 폭부터 영어에서도 겹침·클리핑·접근 불가가 없어야 한다.
- 기존 다크 디자인과 팔레트는 유지한다. 라이트 팔레트는 텍스트, placeholder, 버튼, 카드, 선택, 입력 커서/selection, 오류와 창 제어까지 적용한다. 새 라이트 팔레트의 일반 텍스트/배경과 강조 버튼 텍스트는 대비 4.5:1 이상, 커서/포커스 표시는 인접 배경과 3:1 이상이다. 이 수치 기준은 기존 다크 팔레트 변경을 요구하지 않는다. 새 선택 버튼의 문구·선택·포커스는 두 테마에서 모두 읽고 구별할 수 있어야 한다.
- System은 현재 `WindowAppearance`를 따르고 런타임 변경을 관찰한다. VibrantLight/Light는 라이트, VibrantDark/Dark는 다크로 매핑한다. Light/Dark 고정 모드는 OS 변경에 영향받지 않는다. GPUI 0.2.2 로컬 소스의 `Context::observe_window_appearance`, `Window::appearance`, `Window::set_window_title`을 사용 가능하다. 관찰 Subscription을 유지한다.

## Acceptance criteria

| AC-ID | Given / When | Then: 관찰 가능한 기대값 | 검증 및 결과 증거 |
|---|---|---|---|
| LT-01 | 설정 파일 없음 / 시작 | Korean/System 선택, 현재 OS에 해당하는 팔레트, 저널 정상 접근 | unit 기본값·appearance 매핑; integration 누락 설정; e2e 초기 선택. 최종 보고서에 이름·수·로그·SHA 기록 |
| LT-02 | 기존 UI / English 및 Korean 선택 | 모든 앱 문구·placeholder·제목·안내를 즉시 선택 언어로 표시; 콘텐츠는 번역하지 않음 | unit 번역 매핑/오류 대표 범주; e2e 실제 버튼 클릭, 빈 입력 placeholder, 창 제목, 저장/날짜 오류, 기존 오류의 언어 전환. 리뷰 문자열 전수 확인 |
| LT-03 | 미저장 draft와 중간 커서/selection, 검색/날짜/편집·이동 상태 / 언어·테마 반복 변경 | 해당 상태와 입력 Entity, 저널 모델·파일 보존; 이후 입력이 기존 커서/selection 위치에 적용 | e2e 실제 입력과 전환 후 typing, 검색·선택 날짜·편집/이동 상태 비교; integration 저널 bytes 비교 |
| LT-04 | 각 테마 모드 / System appearance 변경 | Light/Dark는 고정; System만 즉시 새 appearance 팔레트 적용; 선택 모드는 그대로 | unit 3모드 × 4appearance 매핑; e2e production 공통 appearance 처리 경로; reviewer observer 등록·Subscription 유지 확인. GPUI TestWindow는 OS 이벤트를 생성하지 않으므로 자동 테스트를 OS 이벤트 관찰로 주장하지 않음; 가능한 native Windows 관찰은 별도 증거, 미수행이면 한계 명시 |
| LT-05 | English/Dark 등 선택 후 / 창 재생성·앱 재시작 | 별도 settings.json에서 선택 복원; 저널 schema/내용 불변 | integration roundtrip·경로 격리; e2e view 재생성 후 선택/문구/팔레트; 설정/저널 bytes 증거 |
| LT-06 | malformed JSON/unknown enum/unreadable settings 또는 저장 경로 실패 / 시작·선택 변경 | 저널 정상 접근, 현지화 오류 안내; 잘못된 원본 파일 보존, 쓰기 실패 시 정상 설정/저널 보존; 선택은 세션에서 사용 가능 | integration invalid/unknown/path-failure fixtures와 원본 bytes 비교, e2e fallback 및 안내. OS permission 조작 대신 디렉터리 충돌로 결정적 실패 유도 |
| LT-07 | Korean/English, 600/768/1024/1360px 폭 / 메뉴·선택·창 제어 조작 | 설정 접근 가능, 버튼·창 제어가 viewport 안에 있고 서로 겹치지 않음; 상단 drag 영역이 설정 클릭 가로채지 않음 | e2e production debug bounds/click과 메뉴 개폐; 각 언어·폭에서 선택 및 window-controls bounds 증거 |
| LT-08 | 마우스 없이 / 설정으로 포커스 이동 후 활성화 | 모든 5개 선택에 도달 가능, Enter/Space 또는 명시된 표준 키 동작으로 선택, 포커스·선택이 구별됨 | e2e keystroke 기반 도달·활성화, focus 상태 관찰; reviewer focus 스타일 확인 |
| LT-09 | Light/Dark / 입력·버튼·오류·선택 렌더 | 모든 영역에 동일 팔레트 적용, 기존 dark 기본 색상 유지, 새 라이트 팔레트의 수치 대비 기준 충족; 새 선택 버튼은 두 테마에서 가독성과 선택·포커스 구별 가능 | unit light palette contrast 계산 및 dark 기본 색상 보존; e2e 입력/루트 theme 동기화; reviewer 색상 사용 및 새 컨트롤 스타일 확인. 가능하면 native screenshots로 light/dark 레이아웃 관찰 |

## 작업/병렬화 계획

| 작업 | Owner | 수정 경로 | 선행 / 산출물 |
|---|---|---|---|
| 구현 | code /root/code | src/*.rs, 필요한 README/DESIGN 문서; Cargo 설정 변경은 불필요 | READY spec / 구현과 checkpoint |
| 테스트 | test /root/test | tests/bujo_unit.rs, tests/bujo_integration.rs, tests/bujo_e2e.rs, .artifacts/ 검증 증거 | 공개 인터페이스 합의 / AC별 자동 테스트와 직접 실행 증거 |
| 리뷰 | reviewer /root/reviewer | .artifacts/ 리뷰 보고서만 | 동일 checkpoint / 독립 PASS 또는 actionable findings |

공통 파일·lockfile owner는 /root/code. spec owner는 본 파일만 수정한다. Test는 읽기 전용 조사 및 테스트 작성만 병렬 수행하고 제품 writer 종료 전 검증하지 않는다. 마지막 전체 format은 모든 writer 종료 후 code만 실행한다.

## 검증 환경과 명령

`.agents/verification.env`의 format, clippy, bujo_unit, bujo_integration, bujo_e2e를 모두 사용한다. 테스트는 기존 suite에 추가해 gate에 수집되도록 한다. Test fixture는 tempfile 디렉터리에 저널과 settings를 함께 생성하고 자동 정리한다. 시스템 설정은 테스트 중 실제 사용자 OS 설정을 바꾸지 않고 공통 appearance handler를 검증한다.

Builder가 `bash scripts/format.sh --write`와 checkpoint commit을 끝낸 후 verifier가 깨끗한 revision에서 `bash scripts/verify.sh`를 직접 실행한다. 각 단계 종료 코드, 테스트 수(0개 금지), 로그 경로와 동일 SHA를 기록한다. AC 보고서는 각 행의 테스트·관찰 결과와 리뷰 소견을 연결하고, 자동 검증/소스 확인/native 관찰을 구별한다. 수동 확인이 불가능한 항목은 실제 한계를 적고 근거 없이 PASS라고 하지 않는다.

테스트용 공개 관찰 제안: 현재 settings와 resolved palette 조회; 입력 placeholder 조회 또는 production render 관찰; 언어를 입력으로 받는 오류 presentation; OS observer와 동일한 appearance 변경 처리 경로. 테스트 전용 명령으로 UI 클릭 검증을 대체하지 않는다. 정확한 API는 code/test가 하나로 합의한다.

## 질문과 가정

- v2 변경: 기존 다크 팔레트 보존과 새 라이트 대비 검증의 적용 범위를 명확히 분리했다. 사용자 승인 범위의 기능 동작은 동일하다.
- 필수 질문: 없음.
- 가정: 같은 디렉터리의 저널은 settings.json을 공유하고 별도 디렉터리로 --data-file을 지정하면 격리된다. 기존 기본 한국어 UX를 유지한다. English에서도 ISO 날짜 입력 형식은 동일하다.
- 위험/rollback: GPUI TestWindow는 OS appearance 변경 callback을 제공하지 않아 native 전달 자체는 source/native 관찰로 따로 보고해야 한다. 저널 스키마를 바꾸지 않으므로 변경 revert가 데이터 rollback을 요구하지 않는다. settings.json은 저널과 별도이다.

## 완료 조건

모든 AC 증거, 5단계 tool PASS, 동일 revision의 독립 reviewer PASS, Builder != Verifier, clean tree를 orchestrator가 확인한다. 변경 시 버전을 올리며 실패를 기준 약화로 해결하지 않는다.
