# Android 구조와 리팩토링 기록

## 분석 결과

기존 `Journal`은 불변 데이터와 검증 규칙을 갖고 있었지만 생성·이월 메서드가
UUID를 직접 생성하여 같은 입력으로 같은 결과를 얻을 수 없었다.
`Store.kt`에는 설정 모델, JSON 변환, 파일 I/O, 백업·충돌 정책, 메모리 세션이
함께 들어 있었다. `JournalViewModel`은 파일 경로를 받아 저장소를 직접 생성하고
로드 실패 정책까지 처리했다. Compose 화면은 데이터 계층의 설정 타입을 사용하고
저널 변경 람다를 ViewModel에 전달했다. 이 때문에 UI와 파일 구현을 분리하여
유스케이스를 검증하기 어려웠다.

## 현재 구조

```text
android-app/
  core/                          Android 의존성이 없는 Kotlin/JVM 모듈
    src/main/kotlin/app/stillnote/
      domain/
        Journal.kt               불변 모델, 검증, 상태 변경, 검색
        Settings.kt              설정 값과 열거형
      application/
        JournalCommand.kt        명시적 명령과 순수한 상태 전이
        Repositories.kt          저장소·ID 생성 포트와 경계 오류
        JournalService.kt        초기 로드, 명령 실행·저장, 설정 저장
        Session.kt               저장 성공 후에만 갱신하는 동기식 세션
  app/src/main/kotlin/app/stillnote/
    data/
      JournalCodec.kt            데스크톱 v1 JSON ↔ 도메인 변환
      SettingsCodec.kt           설정 JSON ↔ 도메인 변환
      StrictJson.kt              JSON 구문·중복 키·유니코드 검증
      FileAccess.kt              읽기, fsync, 원자적 파일 교체
      JournalStore.kt            로드 보호, 충돌 검사, 이전 파일 백업
      SettingsStore.kt           설정 로드 보호·충돌 검사
      SameBytes.kt               순수한 바이트 비교
    presentation/
      JournalViewModel.kt        화면 상태, SavedState, 코루틴·명령 직렬화
    ui/                          Compose 화면과 테마
    di/JournalDependencies.kt    파일 저장소와 UUID 생성기 조립
    MainActivity.kt              Android 진입점과 ViewModel 연결
```

컴파일 의존성은 `app → core`이다. `core`는 Android, Compose, 파일 API,
JSON 라이브러리, 코루틴을 참조하지 않는다. 내부에서는 `application → domain`으로
의존한다. `data`는 application의 저장소 인터페이스를 구현하고,
presentation은 구체 저장소 대신 `JournalService`를 호출한다.
`ui`는 domain과 presentation/application의 명령을 사용한다.
구체 구현 생성은 `di`에서 한다.

Gradle 모듈 경계가 core에서 app 구현을 참조하는 것을 막는다. app 내부의
`ui`/`presentation`에서 `data`/`di`를 참조하지 않는 규칙은 패키지 규칙이며,
별도 Gradle 모듈로 강제하지는 않는다. 작고 긴밀하게 연관된 유스케이스는 하나의
서비스로 묶고, 저장 포트는 저널과 설정의 독립적인 실패 정책에 맞춰 나누었다.

## 순수한 코드와 부수효과

`Journal.addEntry`, `addCollection`, `migrate`는 이제 ID를 필수 인자로 받는다.
`Journal.apply(command, newId)`는 저장·UUID 생성 없이 새 저널만 반환하며
입력 저널은 수정하지 않는다. 검색, 날짜 검증, JSON 변환도 결정적인 계산이다.

`JournalService`는 **부수효과를 조정하는 코드**이다. 필요한 생성 명령에만
`IdGenerator`를 호출하고, 순수한 상태 전이 결과를 저장한 뒤 반환한다.
실제 UUID 생성과 파일 I/O는 app의 어댑터에서 일어난다.
ViewModel의 디스패처도 주입할 수 있어 테스트에서 실제 파일과 스레드 없이
상태 전이·실패 정책을 확인한다. Android 생명주기, SavedState, Compose의 상태,
현재 날짜 조회, 시스템 창 설정은 app의 프레임워크 경계에 남는다.

## 보존한 정책

- 최초 실행은 빈 저널이며 시드 파일을 쓰지 않는다.
- 파일을 정상 로드하기 전에는 저장을 허용하지 않는다.
- 외부 변경·삭제는 baseline 비교로 감지하며 덮어쓰지 않는다.
- 저널은 이전 파일을 백업하고 임시 파일을 fsync한 뒤 원자적으로 교체한다.
- 변경은 저장 성공 후 화면에 게시하며 성공 콜백도 그때만 실행한다.
- 손상된 저널은 변경을 막고, 손상된 설정은 정상 저널 사용을 막지 않는다.
- 설정 저장 실패 시 현재 세션의 선택과 원본 파일을 유지한다.
- 코루틴 취소는 저장 오류로 변환하지 않고 다시 전파한다.
- 초기 로드와 이후 명령·설정 저장은 같은 Mutex로 직렬화한다.
- 데스크톱 JSON v1과 파일 이름, UI 테스트 태그를 유지한다.

## 검증

`core`의 도메인 테스트와 가짜 저장소 기반 유스케이스 테스트,
`app`의 기존 파일 장애·충돌·JSON 보호 테스트 및 새 ViewModel 테스트를 사용한다.
화면 테스트는 기존 시나리오를 유지하고 생성 부분만 새 의존성 조립 방식으로 바꿨다.
검증 스크립트에도 `:core:test`를 추가했다.

```powershell
./gradlew.bat spotlessCheck :core:test testDebugUnitTest lintDebug assembleDebug
cargo run --locked --manifest-path app/src/test/rust-fixture/Cargo.toml -- validate app/build/compatibility-roundtrip.json
# ANDROID_SERIAL은 격리된 API 36 에뮬레이터를 지정한다.
./gradlew.bat connectedDebugAndroidTest
```

`./gradlew.bat :core:test`만으로 Android 장치 없이 핵심 규칙을 검증할 수 있다.
루트 빌드는 Android 플러그인을 구성하므로 Android SDK가 설치된 개발 환경에서
실행한다. `docs/verification`의 기존 보고서·manifest는 이전 검증 시점의 기록이며
현재 변경의 근거는 이번 실행으로 생성된 `core/build` 및 `app/build` 보고서이다.
이번 실행 요약과 첫 실패·전용 환경 재실행 결과는
[리팩토링 검증 보고서](verification/refactoring-report.md)에 기록했다.

새 기능은 먼저 domain에 불변 규칙을 구현하고 application에 명령과 필요한 포트를
추가한다. 저장·운영체제 구현은 data 또는 프레임워크 경계에서 제공하고 di에서
연결한다. 파일 경로나 Android 타입을 core에 전달하지 않는다.
