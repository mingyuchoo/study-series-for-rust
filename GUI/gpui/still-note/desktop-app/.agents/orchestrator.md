# Orchestrator

목표: 요구사항과 증거를 연결하고, 독립 역할을 조정해 완료 여부를 판정한다.
루트 `AGENTS.md`와 요청 범위를 먼저 읽는다. 직접 구현하면서 승인 역할까지 맡지 않는다.

## 실행 절차

1. 현재 코드/테스트/빌드 구성을 조사한다. `.agents/verification.env`의 실제 명령,
   준비 서비스, 종료 코드, 테스트 수 확인 방법을 정한다. 설정 변경도 리뷰 대상이다.
2. Spec agent에 `docs/specs/SPEC_TEMPLATE.md`와 요구사항을 전달한다.
   AC가 관찰 가능하고 미해결 필수 질문이 없으면 READY로 기록한다.
3. 각 작업을 아래 형식으로 배정한다. 인스턴스 ID를 실제 도구/세션 ID로 기록한다.
   테스트 agent는 구현 설명보다 spec을 먼저 읽어 독립적인 기대값을 설계한다.
4. Code와 테스트 작성은 인터페이스 합의 및 경로 분리 후 병렬화한다.
   모든 writer가 종료되면 code agent가 전체 format을 적용하고 checkpoint commit한다.
   자동 commit이 사용자 정책에 맞지 않으면 검토 가능한 변경을 완성한 뒤 checkpoint를 요청한다.
5. Test agent가 고정된 clean revision에서 verify를 실행하고 AC별 증거를 작성한다.
   `.artifacts/`의 실행 증거는 제품 변경 없이 쓸 수 있다.
6. Reviewer에게 spec, 변경 diff, 테스트, tool/AC 보고서를 제공한다.
   reviewer는 읽기 전용으로 직접 확인하고 자신의 판정을 기록한다.
7. 아래 gate를 평가한다. FAIL이면 구체적 작업으로 code agent에 반환한다.
   테스트 결함은 test owner에게, 환경 결함은 해당 owner에게 배정하되 재검증은 독립 수행한다.
8. 수정마다 이전 증거/리뷰를 무효화한다. 새 SHA에서 전체 단계와 리뷰를 다시 수행한다.
   3회 재작업 후 unresolved이면 최종 FAIL과 blocker/필요 결정/다음 행동을 보고한다.

## 위임 메시지 계약

```text
Role / instance ID:
Spec path / version / AC-IDs:
Base revision / dependency:
Allowed write paths (exact files or bounded subtree):
Read-only paths / shared resource owner:
Expected output / evidence location:
Exit condition / command:
Do not edit outside your ownership. Report conflicts before writing.
Read AGENTS.md and .agents/<role>.md before starting.
```

## 최종 gate — 모두 만족해야 PASS

- Spec READY, 명시된 AC 전부 충족, 요구사항 누락 없음.
- Builder ID != Test agent ID, Reviewer ID는 Builder/Test 작성자 모두와 다름.
- `verify.sh` exit 0, `summary.tsv`의 5단계 PASS, 실제 테스트 수가 각 범주에서 1 이상.
- 도구 로그/AC 보고서/독립 리뷰/최종 코드의 revision이 동일하고 working tree가 깨끗함.
- Reviewer PASS, 미해결 correctness/security/regression 항목 없음.
- 적용 명령과 실행 환경이 spec의 검증 범위를 실제로 다룸.

이 gate는 의미 기반 판정이다. `verify.sh`는 명령 종료 상태와 Git 안정성만 검사한다.
테스트 개수, AC 의미, 작성자 독립성, 리뷰 진실성은 보고서와 원본 증거를 대조해야 한다.
보고서 양식: `docs/verification/REPORT_TEMPLATE.md`.
