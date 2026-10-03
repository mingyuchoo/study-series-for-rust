# Kotlin Toolchain 전환 검증

> Historical verification snapshot. Results, source hashes and paths below apply only to that run.
> Current structure and commands: [architecture](../ARCHITECTURE.md), [README](../../README.md).

검증일: 2026-10-03, Windows 11.

## 전환 내용

- 공식 `kotlin update --target-version=0.13.0`으로 받은 래퍼를 사용한다.
  배포 SHA-256은 `fa82b9872dcfa04d26c8f800f33d597c99fbffd747a0e9bbce3a9e4424b3a588`이다.
- `project.yaml`과 app/core/tooling의 `module.yaml`이 빌드 정의이다.
  Kotlin 2.2.21, JDK/바이트코드 17, Android SDK 36, Build Tools 36.0.0,
  Compose BOM 2025.10.01 및 기존 AndroidX 의존성 버전을 유지한다.
- 직접 관리하던 root/app/core Gradle 스크립트, Gradle properties와 래퍼를 제거했다.
  제거 후 `kotlin clean`, `kotlin build`, `kotlin test`와 Rust 검증을 다시 실행하여 통과했다.
- app은 기본 레이아웃, core는 Maven-like 레이아웃이다. 제품 패키지명은 그대로다.
  이동한 제품 Kotlin 12개, 리소스 5개, JVM 테스트 2개, JSON fixture 1개,
  UI 테스트 2개는 전환 전 Git 소스와 비교하여 줄바꿈 외 내용 변경이 없음을 확인했다.
- Rust harness를 `scripts/rust-fixture`로 옮기고 실제 desktop 모델을 참조하는
  상대 경로를 수정했다. 독립 Cargo.lock은 유지한다.
- 포맷 검증은 동일한 ktfmt 0.54 Kotlin 스타일을 사용한다.
  Windows 래퍼의 CRLF와 shell 래퍼의 LF를 `.gitattributes`로 고정했다.

## 확인한 결과

| 검증 | 결과 |
| --- | --- |
| `kotlin build` | 성공, CLI Android APK 생성 |
| `kotlin test` | core 13개 + app 18개, 실패/건너뜀 0개 |
| `kotlin test -m core`, Android SDK 환경변수 제거 | 13개 통과 |
| CLI 테스트 출력 JSON의 실제 Rust desktop 모델 검증 | 성공 |
| ktfmt 검사 | 성공 |
| Android lint | 오류 0개, 경고 14개 |
| CLI APK 설치·MainActivity 실행 | 전용 API 36 에뮬레이터에서 성공 |
| CLI 제품 JAR을 사용하는 Compose UI 테스트 | 14개 통과, 실패/건너뜀 0개 |
| APK metadata | app.stillnote, versionCode 1, versionName 1.0, compile/min/target 36 |

JVM 결과는 [unit-tests.log](toolchain-migration/unit-tests.log), UI는
[final-ui.xml](toolchain-migration/final-ui.xml), lint는
[lint-results-debug.xml](toolchain-migration/lint-results-debug.xml)에 보존했다.
lint 경고는 기존 의존성/SDK/아이콘/백업 설정 및 검증용 생성 프로젝트의
Gradle 버전·외부 경로 관련 경고이다. 마이그레이션 중 라이브러리를 함께
업그레이드하지 않았다.

## Android 검증 연결

Kotlin Toolchain 0.13.0은 Android 패키징에 Gradle/AGP를 내부 사용한다.
공개 CLI에 lint와 connected instrumentation 명령이 없어 `tooling` JVM 모듈이
`build/android-checks`에 검증 프로젝트를 생성한다. 연결 프로젝트의
Gradle 8.13/AGP 8.13.2는 기존 검증 버전이며 제품 설정과 의존성은 app YAML에서 읽는다.

lint는 실제 제품 소스와 리소스를 분석한다. 화면 검증 호스트는
`app.stillnote.verification`이며 Kotlin CLI의 app/core JAR과 제품 리소스를 사용한다.
`compileDebugKotlin`은 비활성화되어 제품 Kotlin을 다시 컴파일하지 않는다.
기존 ComponentActivity 테스트를 위해 Compose test manifest를 이 호스트에만 추가한다.
`ui` 명령은 먼저 실제 CLI APK를 설치·실행한 뒤 호스트의 14개 UI 테스트를 실행한다.
물리 기기와 API 36 외 에뮬레이터를 거부하고, 실패·건너뜀·결과 누락을 오류로 처리한다.
선택한 검증 에뮬레이터는 화면을 켠 상태로 유지하고 잠금 화면을 해제한다.

첫 UI 실행은 첫 번째 테스트가 통과한 후 에뮬레이터 연결이 사라져 중단됐다.
전체 완료 결과로 취급하지 않으며 [중단 기록](toolchain-migration/interrupted-ui.xml)을 보존했다.
다음 실행은 14개 중 10개가 소프트 키보드 표시 대기에서 실패했다.
[실패 기록](toolchain-migration/ime-failed-ui.xml)을 보존했다. 잠금 화면을 명시적으로
해제한 후 같은 APK와 변경하지 않은 입력 테스트 1개가 통과했으며,
검증 어댑터에 깨우기·잠금 해제 절차를 추가한 후 전체 14개 테스트가 통과했다.
제품 코드와 테스트 구현·타임아웃은 변경하지 않았다.

## 범위와 근거

이 전환은 기존 도메인·저장 정책·UI 구현을 변경하지 않는다.
기존 `docs/verification` 보고서와 source manifest는 과거 실행의 기록으로 유지한다.
이번 검증은 debug APK와 자동 테스트를 대상으로 한다. Release 빌드·서명·배포와
물리 기기 TalkBack/키보드 검증은 수행하지 않았다. Toolchain 0.13의 내부 release
빌드는 R8 및 리소스 축소를 켜므로 배포 전 별도 release 검증이 필요하다.

공식 문서: [Gradle 전환](https://kotlin-toolchain.org/latest/getting-started/migrating-from-gradle/),
[Android 제품](https://kotlin-toolchain.org/latest/user-guide/product-types/android-app/),
[래퍼](https://kotlin-toolchain.org/latest/cli/provisioning/).
