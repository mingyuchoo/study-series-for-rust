# Android Clean Architecture

## 분석과 이번 변경

기존 코드에는 이미 `app → core` 모듈 경계, 불변 도메인, 명시적 변경 명령,
저장소 포트와 DI 조립부가 있었다. 도메인에 UUID를 주입하는 이전 리팩토링도
유지했다. 이번에는 남아 있던 결합과 중복을 다음과 같이 정리했다.

| 기존 책임 혼합 | 변경 후 |
| --- | --- |
| ViewModel이 구체 JournalService를 참조 | JournalUseCases 입력 포트에 의존 |
| Compose 화면에서 ViewModel·생명주기·SavedState·시계 접근 | StillnoteRoute가 프레임워크 연결, StillnoteScreen은 상태와 계약을 전달받음 |
| 화면 안에서 검색·필터·인덱스 계산 | core의 JournalQuery와 indexLocations 순수 조회 |
| 저널과 설정 저장소에 로드 보호·baseline·충돌·저장 처리 중복 | ProtectedFileStore가 공통 파일 정책 소유, 각 저장소는 모델별 변환과 백업 선택 |
| 내부 패키지 의존성 규칙이 문서에만 존재 | tooling architecture 명령과 verify 스크립트에서 소스 규칙 검사 |

## 구조와 의존성

```text
core/src/main/kotlin/app/stillnote/
  domain/
    Journal.kt                 불변 모델, 검증, 변경, 검색 규칙
    Settings.kt                설정 값
  application/
    JournalCommand.kt          명시적 변경 명령과 순수 전이
    JournalQuery.kt            순수 목록 조회와 위치별 인덱스
    JournalUseCases.kt         입력 포트와 로드 결과
    Repositories.kt            저장소·ID 생성 출력 포트
    JournalService.kt          로드·ID 생성·저장 조정
app/src/app/stillnote/
  data/
    JournalStore.kt             저널 어댑터, 검증·이전 파일 백업 선택
    SettingsStore.kt            설정 어댑터
    ProtectedFileStore.kt       로드 보호·충돌 검사·내구성 baseline
    FileAccess.kt              읽기·fsync·원자적 파일 교체
    JournalCodec.kt             데스크톱 v1 JSON 변환
    SettingsCodec.kt            설정 JSON 변환
    StrictJson.kt, SameBytes.kt 순수 구문 검증·바이트 비교
  presentation/
    JournalViewModel.kt         코루틴·명령 직렬화·화면 상태 게시
    UiState.kt                  불변 화면 상태
    JournalScreenContract.kt    화면 이벤트·복원 상태 접근 계약
  ui/
    StillnoteRoute.kt           생명주기 수집·SavedState 연결·시계 제공
    StillnoteScreen.kt          화면 렌더링·편집 상태·사용자 이벤트
    Theme.kt                    Compose 테마
  di/JournalDependencies.kt     파일 저장소와 실제 UUID 생성기 조립
  MainActivity.kt               Android 진입점과 창 설정
```

컴파일 의존성은 `app → core`, core 내부에서는 `application → domain`이다.
presentation은 입력 포트에 의존하고 data는 출력 포트를 구현한다.
서비스는 저장소 구현을 모르며, 화면은 ViewModel과 파일 저장소를 모른다.
구체 구현 생성은 DI 조립부에서만 수행한다.

`JournalService`는 연관된 저널·설정 사용 흐름을 조정한다. 파일 정책은 하나의
내부 객체로 모으고, 모델별 저장소는 작은 어댑터로 유지했다. JSON과 파일 효과를
도메인에 넣지 않았다. 기존 두 제품 모듈을 유지하여 작은 기능마다 모듈이나
유스케이스 클래스를 추가하지 않았다.

## 순수 계산과 부수효과

순수 계산은 도메인 검증·변경·검색, `Journal.apply(command, newId)`,
`JournalQuery.entries`, `Journal.indexLocations`, JSON 변환과 바이트 비교다.
변경 함수는 전달받은 ID만 사용하고 입력 모델을 수정하지 않는다.
인덱스는 일간의 날짜, 월간·미래의 연월, 컬렉션 ID로 묶어 입력 순서를 유지한다.
검색은 현재 위치와 인덱스 선택보다 우선하며 상태 필터를 적용한다.

서비스는 출력 포트를 호출하는 효과 조정 코드다. 저장소 어댑터는
실제 파일 효과를 수행한다. ViewModel은 코루틴·Mutex·StateFlow·SavedState를,
Route와 Compose 화면은 생명주기·UI 상태 효과를 담당한다. Compose 화면 자체는
순수 함수라고 주장하지 않는다. 날짜 공급 함수를 주입하므로 화면을 실제 시계나
ViewModel 없이 구성할 수 있고, 성공 이벤트는 저장이 끝난 저널을 전달받는다.

## 보존한 정책

- 최초 실행은 빈 저널이며 파일을 미리 쓰지 않는다.
- 정상 로드 전에 저장하지 않고, 손상된 원본을 보호한다.
- 외부 변경·삭제를 baseline 비교로 감지하고 덮어쓰지 않는다.
- 저널만 이전 파일을 백업하고, 임시 파일 fsync 후 원자적으로 교체한다.
- baseline과 화면 저널은 저장 성공 후 갱신한다. 저장 실패 시 초안을 유지한다.
- 설정 실패는 정상 저널을 막지 않으며, 설정 선택은 현재 세션에 적용한다.
- 초기 로드·명령·설정 저장은 같은 Mutex로 직렬화한다.
- 코루틴 취소를 저장 오류로 바꾸지 않는다.
- JSON v1, 파일 이름, 화면 테스트 태그와 SavedState 키를 유지한다.

## 경계 검사와 검증

`kotlin run -m tooling -- architecture`는 core의 Android·JSON·파일 의존성,
도메인의 application 참조, presentation/data/UI의 역방향 참조 및 core의 직접
UUID·시계 호출을 검사한다. UI의 생명주기 연결은 Route에 둔다. 이 검사는 소스
정규식 기반 보조 검사이며 모든 Kotlin 별칭·와일드카드·간접 호출을 분석하는
컴파일러 플러그인은 아니다. core에서 app을 참조하지 못하는 것은 모듈 컴파일이
강제한다. verify.ps1과 verify.sh에 이 검사를 포함했다.

```powershell
./kotlin.bat run -m tooling -- format
./kotlin.bat run -m tooling -- architecture
./kotlin.bat build
./kotlin.bat test
cargo run --locked --manifest-path scripts/rust-fixture/Cargo.toml -- validate build/compatibility-roundtrip.json
./kotlin.bat run -m tooling -- lint
# 격리된 API 36 에뮬레이터에서만 실행
./kotlin.bat run -m tooling -- ui
```

새 조회 테스트는 전역 검색·필터 우선순위·빈 인덱스·날짜와 로그별 인덱스 그룹을
검증한다. ViewModel의 추가 테스트는 저장소 없이 입력 포트를 주입하고 취소 시
저널·오류·busy·성공 콜백을 검증한다. 기존 파일 장애·충돌·손상 보호 테스트는
공통 파일 경계로 옮긴 정책을 계속 검증한다.

이번 실행 결과는 [2026-10-04 검증 기록](verification/clean-architecture-report.md)에
기록했다. 기존 verification 문서들은 각 실행 시점의 역사적 기록으로 유지한다.

저장소 테스트에서만 쓰는 StoreSession은 app/test에 있으며 제품 API에 포함하지 않는다.
