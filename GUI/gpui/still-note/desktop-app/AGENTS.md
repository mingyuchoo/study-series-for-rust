# Repository multi-agent contract

이 저장소의 기능 구현은 아래 계약을 따른다. 사용자 지시와 실제 실행 환경의
상위 지시가 우선한다. `.agents/*.md`는 역할 지침이며 자동 등록되는 에이전트가 아니다.
Orchestrator는 위임 시 루트 계약과 해당 역할 파일을 명시적으로 읽게 한다.

## 필수 흐름

Spec → Code → Tool-based format/lint/test → Independent Review → PASS/FAIL gate.
구현 요청에 대해 orchestrator는 별도 code, test, reviewer subagent에 위임한다.
작은 작업은 spec 역할을 orchestrator가 맡아도 되지만 구현자와 검증자는 분리한다.
subagent를 사용할 수 없으면 그 제한을 보고하고 별도 세션 또는 사람의 독립 검증을
기다린다. 같은 에이전트가 역할 이름만 바꾸어 독립 검증했다고 주장하지 않는다.

- Builder != Verifier: 제품 코드 작성자는 자신의 변경에 대한 최종 검증/리뷰 승인자가 아니다.
- Code agent의 자체 테스트는 진단용이다. Test agent가 도구를 직접 실행하고 증거를 남긴다.
- Reviewer는 해당 변경의 제품 코드와 테스트를 작성하지 않은 별도 에이전트/사람이다.
- Spec의 AC-ID마다 기대 동작, 테스트/관찰 방법, 결과 증거가 있어야 한다.
- 필수 단계 미설정, 미실행, 실패, 수집 테스트 0개, 미해결 리뷰 항목은 최종 FAIL이다.
- 테스트 삭제, assertion 약화, skip 추가, lint 예외로 실패를 숨기지 않는다.

## 역할과 수정 권한

| 역할 | 읽을 지침 | 수정 가능 범위 |
|---|---|---|
| orchestrator | `.agents/orchestrator.md` | 작업 배정, 상태/판정 보고서 |
| spec | `.agents/spec-agent.md` | 배정된 `docs/specs/*.md` |
| code | `.agents/code-agent.md` | 배정된 제품 코드, 승인된 설정/문서 |
| refactoring (code 계열 Builder) | `.agents/code-agent.md`, `.agents/refactoring-agent.md` | 배정된 리팩토링 대상 제품 코드, 승인된 설정/문서 |
| test | `.agents/test-agent.md` | 배정된 테스트/fixture, 검증 증거 |
| reviewer | `.agents/reviewer-agent.md` | 리뷰 보고서만 |

순수 코드/부수효과 분리, 응집도/결합도 개선, Clean Architecture 리팩토링 요청은
Code 단계의 Builder를 refactoring 역할로 배정한다. 루트 계약과 두 역할 지침을
함께 읽게 하고, READY spec의 AC와 경로 owner를 전달한다. 이 역할은 code 역할의
전문화이며 별도 승인 단계가 아니다. 독립 test/reviewer의 배정, 수정 권한,
동일 revision 검증과 최종 gate는 그대로 필수다.

## 병렬 작업

1. Orchestrator가 경로별 owner, 읽기/쓰기 범위, 의존성, 산출물을 배정한다.
2. 같은 파일 동시 수정 금지. lockfile, 공통 설정, 생성 파일도 하나의 owner만 가진다.
3. 인터페이스 확정 후 서로 다른 제품 파일과 테스트 파일의 구현은 병렬화 가능하다.
4. 읽기 전용 조사/테스트 설계는 병렬화 가능하다. 공유 코드 수정 중 검증은 금지한다.
5. 전체 format은 모든 writer 종료 후 code agent가 단독 실행한다.
6. 테스트 병렬 실행은 별도 worktree/출력 폴더/포트/DB로 격리한 경우만 허용한다.
   기본 `verify.sh`는 순차 실행한다. 통합 후 최종 revision을 다시 검증한다.
7. 경로 충돌은 먼저 orchestrator에게 알리고 owner를 이전한다. 타인의 변경을 덮어쓰지 않는다.

## 검증과 재작업

- `.agents/verification.env`에 저장소의 실제 명령을 설정한다. 모든 단계가 필수다.
- Builder: `bash scripts/format.sh --write`로 포맷 후 변경을 checkpoint commit한다.
- Verifier: 깨끗한 Git revision에서 `bash scripts/verify.sh`를 실행한다.
- 도구 PASS는 최종 PASS가 아니다. AC 검증 보고서와 독립 리뷰가 같은 SHA를 가리켜야 한다.
- 실패 시 실패 명령/종료 코드/로그/AC-ID/재현 방법을 code agent에게 반환한다.
  수정 → format → 새 revision → 전체 verify → 새 독립 리뷰 순으로 반복한다.
- 기본 재작업 한도는 3회다. 같은 원인 반복 또는 한도 도달 시 FAIL과 blocker를 보고한다.
  spec 변경이 필요하면 spec 단계로 돌아가 기준을 갱신한다. 기준을 몰래 낮추지 않는다.
- 사용자의 별도 승인이 요구되지 않는 범위에서는 spec READY로 바로 진행한다.
  제품 의도/파괴적 동작 등 답이 필수인 질문만 사용자에게 묻는다.

## 완료 보고

Revision, spec 버전, Builder/Test/Reviewer ID, 도구 실행 디렉터리,
AC별 증거, 리뷰 판정, 최종 PASS/FAIL, 남은 위험을 적는다.
PASS 자체는 merge, 배포, 외부 메시지 발송의 승인을 의미하지 않는다.
