# Android Clean Architecture

## 분석 결과와 리팩토링 범위

기존 `core`에는 불변 저널 모델, 명시적 명령, 순수 조회, 저장소 포트와 효과를
조정하는 서비스가 이미 있었다. 파일 저장소도 원자적 교체·백업·충돌 보호를
`ProtectedFileStore`에 모아 두고 있었다. 이 경계는 유지했다.

남아 있던 문제는 Android 모듈 안의 순수 화면 계약과 상태 모델, 화면 함수 안의
조회·문구·날짜 이동 계산, 큰 화면 함수에 모여 있던 서로 다른 UI 책임이었다.
이번 변경은 그 책임을 JVM `presentation` 모듈과 역할별 Compose 컴포넌트로
분리한다. 바로 앞에서 변경한 화면 순서와 미래 로그 입력란 동작도 유지한다.

| 분석한 결합 | 변경 후 책임 |
| --- | --- |
| 화면 계약과 UiState가 Android 모듈에 위치 | `presentation` JVM 모듈에 위치, Android 없이 컴파일·테스트 |
| ViewModel에서 화면 상태 복사와 효과 처리 혼합 | 순수 `UiState.reduce(UiTransition)` + ViewModel의 코루틴·저장 조정 |
| 화면에서 검색과 필터 계산을 중복 실행 | `LogPageModel.project`가 `JournalQuery` 결과를 한 번 계산 |
| 화면에서 월간 달력의 각 날짜마다 저널 탐색 | 순수 `monthlyCalendar`가 날짜별 개수를 집계해 전달 |
| 화면에서 문자열 목적지와 날짜 이동 판단 | `JournalPage`가 기존 저장 키를 유지하며 목적지와 이동 규칙 제공 |
| 기록 카드가 편집 상태·저장 명령·화면 이동을 직접 처리 | `EntryCard`는 `EntryIntent`를 전달하고 화면 연결부가 처리 |
| 한 화면 함수가 모든 입력·검색·설정·대화상자 렌더링 | 각 역할의 컴포넌트로 분리 |

## 모듈과 의존성

```text
app (Android, effects and adapters)
  ├── presentation (JVM, pure screen models and contracts)
  │     └── core (JVM, domain and application ports)
  └── core
```

`presentation`의 제품 의존성은 `core` 하나뿐이다. `core`는 다른 제품 모듈에
의존하지 않는다. Android·Compose·코루틴 의존성은 `app`에만 있다.
두 JVM 모듈은 Android SDK나 에뮬레이터 없이 테스트할 수 있다.

```text
core/src/main/kotlin/app/stillnote/
  domain/                      저널·설정 값, 검증·변경·검색 규칙
  application/
    JournalCommand.kt          전달받은 ID를 사용하는 순수 저널 전이
    JournalQuery.kt            순수 조회와 위치별 인덱스
    JournalUseCases.kt         입력 포트
    Repositories.kt            저장소·ID 생성 출력 포트
    JournalService.kt          포트를 통한 로드·ID 생성·저장 조정
presentation/src/main/kotlin/app/stillnote/presentation/
    UiState.kt                 불변 화면 상태
    UiTransition.kt            순수 화면 상태 전이와 오류 코드 변환
    JournalScreenContract.kt   화면 이벤트·복원 상태 접근 계약
    JournalPage.kt             목적지, 날짜 이동, 달력 집계
    LogPageModel.kt            화면에 필요한 순수 조회 결과
    JournalStrings.kt          언어별 문구와 오류·위치 표시
    EntryIntent.kt             기록 컴포넌트의 사용자 의도
app/src/app/stillnote/
  data/                        JSON·파일 어댑터와 보호된 저장 정책
  presentation/
    JournalViewModel.kt        Android 상태 게시·코루틴·Mutex·SavedState 어댑터
  ui/
    StillnoteRoute.kt           생명주기 수집·ViewModel 연결·시계 제공
    StillnoteScreen.kt          복원 가능한 편집 상태·화면 연결·전체 레이아웃
    EntryCapturePanel.kt       기록 입력
    JournalSearchPanel.kt      검색·필터
    DateNavigationPanel.kt     날짜 입력·이동 버튼
    MonthlyCalendarPanel.kt    달력 렌더링
    EntryCard.kt               기록 표시·사용자 의도 전달
    JournalSettingsPanel.kt    설정·컬렉션 만들기
    JournalDialogs.kt          편집·이월 대화상자 렌더링
    JournalComponents.kt       공통 버튼·패널
    Theme.kt                   Compose 테마
  di/JournalDependencies.kt     실제 파일 저장소와 UUID 생성기 조립
  MainActivity.kt               Android 진입점과 창 설정
```

기존 타입의 패키지 이름은 유지했다. `app.stillnote.presentation` 패키지의
프레임워크 독립 타입은 `presentation` 모듈에, Android ViewModel은 `app` 모듈에
있다. 기존 import와 테스트·SavedState 계약을 유지하면서 모듈 의존성으로 경계를
강제한다. 기능마다 별도 모듈이나 작은 유스케이스 클래스를 만들지는 않았다.

## 순수 계산과 부수효과

순수 계산은 같은 입력으로 같은 결과를 만들고 호출자에게 보이는 상태를 바꾸지
않는다. 도메인 변경·검증·검색, 명령 적용, 페이지 조회, 달력 집계, 날짜 이동,
표시 문구와 화면 상태 전이가 이 범주다. UUID·현재 날짜·저장 결과는 외부에서
전달한다. JSON 변환과 바이트 비교도 파일 어댑터 내의 별도 순수 함수로 유지한다.

효과는 서비스의 저장소·ID 포트 호출, 파일 읽기·백업·fsync·원자적 교체,
ViewModel의 코루틴·Mutex·StateFlow·SavedState, Route의 생명주기·시계,
Compose의 화면 상태·키보드·포커스 처리다. 서비스는 저장소 구현을 모르고,
화면 컴포넌트는 ViewModel이나 파일 저장소를 모른다. 구체 구현은 DI에서 조립한다.

Compose와 ViewModel 자체를 순수 함수로 취급하지 않는다. UI에 필요한 IME 조합,
커서 선택, 페이지 스크롤과 복원 상태는 Android/Compose 경계에 유지한다.
`StillnoteScreen`은 이 상태들의 수명과 사용자 이벤트를 연결한다.

## 유지하는 정책

- 최초 실행은 빈 저널이며 파일을 미리 쓰지 않는다.
- 정상 로드 전 저장을 막고 손상된 원본을 보호한다.
- 외부 변경·삭제를 baseline으로 감지하고 덮어쓰지 않는다.
- 저널만 이전 파일을 백업하며 fsync 후 원자적으로 교체한다.
- baseline과 화면 저널은 저장 성공 후 갱신한다. 실패 시 초안을 유지한다.
- 설정 저장 실패는 저널 사용을 막지 않으며 설정 선택은 현재 세션에 적용한다.
- 초기 로드·명령·설정 저장을 같은 Mutex로 직렬화한다.
- 코루틴 취소는 저장 오류로 보고하지 않는다.
- JSON v1, 파일 이름, 테스트 태그와 SavedState 키를 유지한다.
- 일간·컬렉션은 입력 우선, 월간은 달력 우선, 검색·필터는 목록 바로 위다.
- 미래 로그 입력란은 선택적으로 펼치며 접을 때 초안을 유지하고 저장 성공 후 닫는다.

## 검사와 테스트

`tooling architecture`는 모듈 의존성과 안쪽 방향의 소스 참조를 검사한다.
`core`와 `presentation`에서 Android·코루틴·파일·네트워크 및 직접 시계·UUID
생성을 금지한다. 하위 패키지에도 app의 presentation/data/UI 규칙을 적용한다.
이 검사는 소스 정규식 기반 보조 검사이며 컴파일러 수준의 모든 별칭이나 간접
호출을 분석하지는 않는다. 모듈 간 역방향 참조는 컴파일 의존성도 제한한다.

포맷 검사와 Android lint/UI 브리지에 새 모듈을 포함했다. 브리지는 Kotlin CLI가
컴파일한 app/core/presentation JAR를 사용하고 제품 Kotlin을 다시 컴파일하지 않는다.

```powershell
./kotlin.bat run -m tooling -- format
./kotlin.bat run -m tooling -- architecture
./kotlin.bat build
./kotlin.bat test
cargo run --locked --manifest-path scripts/rust-fixture/Cargo.toml -- validate build/compatibility-roundtrip.json
./kotlin.bat run -m tooling -- lint
# 격리된 API 36 에뮬레이터에서 실행
./kotlin.bat run -m tooling -- ui
```

JVM 테스트는 core 16개, app 19개, presentation 10개다. 새 테스트는 전역 검색과
인덱스·달력 우선순위, 윤년의 날짜별 기록 집계, 복원 키와 날짜 범위, 문구,
저장 실패·성공·취소·설정 선택의 순수 상태 전이를 검증한다. 기존 파일 장애,
충돌, 손상 보호, 명령 직렬화 및 JSON 호환성 테스트를 함께 유지한다.
UI 회귀 테스트는 22개이며 IME·복원·페이지 이동·이월·검색·미래 입력 흐름을 포함한다.
기존 `docs/verification` 문서는 해당 실행 시점의 역사적 기록으로 유지한다.
