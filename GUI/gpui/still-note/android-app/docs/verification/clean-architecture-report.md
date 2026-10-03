# Clean Architecture 리팩토링 검증

> Historical verification snapshot. Results, source hashes and paths below apply only to that run.
> Current structure and commands: [architecture](../ARCHITECTURE.md), [README](../../README.md).

검증일: 2026-10-04, Windows / PowerShell.

## 변경 범위

기존 core/app 모듈을 유지하고 입력 포트, 순수 조회 유스케이스, 화면 계약과
Android Route, 공통 보호 파일 저장 경계를 추가했다. 도메인 규칙, JSON v1,
파일 이름, 화면 테스트 태그와 복원 키를 유지했다. 작업 시작 전 존재하던
아이콘 리소스와 AndroidManifest 변경은 그대로 보존했다.

## 결과

| 검증 | 결과 |
| --- | --- |
| ktfmt 포맷 검사 | 통과 |
| architecture 소스 의존성 검사 | 통과 |
| Kotlin Toolchain Android 빌드 | 성공, Debug APK 생성 |
| JVM 테스트 | core 16개 + app 19개, 총 35개 통과 |
| Rust 실제 desktop 모델 JSON 검증 | 모든 fixture 값과 migration link 검증 통과 |
| Android lint | 성공, 오류 0개 / 경고 14개 |
| git diff --check | 공백 오류 없음 |
| Compose UI instrumentation | 실행하지 않음: 연결된 emulator-5554가 API 37, 검증 계약은 격리된 API 36만 허용 |

추가 테스트 4개는 전역 검색과 필터 우선순위, 빈 인덱스와 위치 조회,
날짜·로그·컬렉션별 인덱스 그룹, 입력 포트 취소 시 화면 게시 방지를 확인한다.
기존 테스트는 백업/원자적 교체 실패, 외부 변경/삭제, 손상 파일 보호,
저장 실패 후 재시도, 직렬화와 설정 실패 정책을 계속 검증한다.

빌드 결과는 `build/tasks/_app_buildAndroidDebug/gradle-project-debug.apk`,
lint 결과는 `build/android-checks/build/reports/lint-results-debug.xml`에 있다.
Rust 검증 입력은 `build/compatibility-roundtrip.json`이다.

UI 테스트 미실행 때문에 실제 Compose 상호작용·프로세스 복원 동작의 이번
실행 결과는 확인되지 않았다. UI 테스트 소스와 태그는 유지했다. 검증 환경을
API 36으로 준비한 뒤 ANDROID_HOME, JAVA_HOME(JDK 17), ANDROID_SERIAL을
설정하고 `./kotlin.bat run -m tooling -- ui`로 확인해야 한다.

## 재현

```powershell
./kotlin.bat run -m tooling -- format
./kotlin.bat run -m tooling -- architecture
./kotlin.bat build
./kotlin.bat test
cargo run --locked --manifest-path scripts/rust-fixture/Cargo.toml -- validate build/compatibility-roundtrip.json
./kotlin.bat run -m tooling -- lint
```

빌드와 lint에는 설치된 Android SDK를 사용했다. lint 어댑터의 JDK 17은
Kotlin Toolchain이 이미 제공한 로컬 캐시 경로를 JAVA_HOME에 지정했다.
사용자 시스템 환경변수나 실행 중인 에뮬레이터 설정을 변경하지 않았다.
architecture 검사는 정규식 기반 보조 검사로, 모듈 컴파일 및 코드 리뷰와
함께 사용한다. 자세한 책임과 의존성은 [ARCHITECTURE.md](../ARCHITECTURE.md)에 있다.
