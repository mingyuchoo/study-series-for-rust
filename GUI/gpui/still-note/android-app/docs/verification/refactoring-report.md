# Clean Architecture 리팩토링 검증

실행일: 2026-10-03 (Asia/Seoul). 구조와 분석은 [ARCHITECTURE.md](../ARCHITECTURE.md)에
기록했다. 기존 독립 검증 보고서와 source manifest는 이전 시점의 기록으로 유지한다.
이번 검증은 같은 에이전트가 수행한 변경 검증이며 독립 리뷰 결과는 아니다.

## 빌드·단위 검증

JDK 17.0.20.1, Gradle wrapper 8.13, Android SDK API 36 환경에서 실행했다.

| 검증 | 결과 |
| --- | --- |
| `spotlessCheck` | 통과 |
| `:core:test` | 13개 통과: 도메인 8개, 유스케이스 5개 |
| `testDebugUnitTest` | 18개 통과: 파일 저장소 15개, ViewModel 3개 |
| `assembleDebug` | 통과 |
| `compileDebugAndroidTestKotlin` | 통과 |
| `lintDebug` | 오류 0, 경고 11; 의존성 버전·target API·기존 manifest 항목 |
| Rust 데스크톱 모델 JSON 검증 | 모든 fixture 값과 이월 링크 검증 통과 |

가짜 저장소 기반 검증은 순수한 명령의 반복 가능성, 원본 불변성, 저장 실패 시
게시하지 않는 정책, 재시도, 설정/저널 오류 우선순위, 잘못된 명령의 저장 방지,
수정 명령의 불필요한 ID 생성 방지, ViewModel 명령 직렬화를 포함한다.
기존 파일 테스트는 UTF-8·JSON·UUID 검증, 원본 보호, 백업, 외부 충돌,
읽기/교체 실패, 병렬 세션 쓰기, 설정 격리와 데스크톱 호환성을 유지한다.

## 화면 테스트 실행 이력

첫 전체 실행은 `emulator-5558 / Stillnote_API_36`, API 36에서 14개 중 13개가
통과했다. `ac05_searchAllLogsStatusFilterIncludesOpenEventsAndNotes`는
`StillnoteFlowTest.enter:118`의 **소프트 키보드 표시 대기**에서 시간 초과했다.
실패 위치는 검색 입력 단계(`:364`)이며 검색 결과 단언에 도달하지 않았다.
에뮬레이터의 `show_ime_with_hard_keyboard` 값은 0이었다.
[첫 실행 XML](refactoring/first-ui-run.xml)을 보존했다.

기존 [검증 보고서](test-report.md)의 환경 조건에 따라 전용
`Stillnote_Verification_Final`을 `emulator-5560`에서 창 없이 실행했다.
부팅 완료, API 36, qemu=1을 확인하고 해당 테스트 에뮬레이터에만
`show_ime_with_hard_keyboard=1`을 적용한 뒤 전체 테스트를 다시 실행했다.
테스트의 대기 시간과 단언은 변경하지 않았다. 물리 장치는 사용하지 않았다.

두 번째 전체 실행은 **14개 통과, 실패·오류·건너뜀 0, Gradle 종료 코드 0**이다.
[최종 실행 XML](refactoring/final-ui-run.xml)을 보존했다. 실행 후 이번 검증에서
시작한 `emulator-5560`만 종료했다. 다른 실행 중인 에뮬레이터는 유지했다.

단위 테스트 31개와 UI 테스트 14개, 형식 검사, lint, APK 빌드,
Rust JSON 호환성 검증이 최종적으로 통과했다. 첫 실행의 키보드 대기 실패는
별도 실패 이력으로 남겼다.
