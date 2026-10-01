# <FEATURE-ID>: <기능 이름>

- Version: 1
- Status: DRAFT / READY (하나 선택)
- Spec owner: <ID>
- Base revision: <SHA>
- Request: <사용자 요청/출처>

## 목표와 범위

현재 동작: <관찰한 문제>
목표: <사용자에게 보이는 결과>
포함: <구현 범위>
제외: <범위 밖 항목>

## 계약과 설계 제약

- API/타입/입출력: <구체적 계약>
- UI/접근성: <관련 요구>
- 오류/경계/보안/호환성: <기대 동작>
- 환경/성능/데이터 변경: <관련 요구 또는 비해당 사유>

## Acceptance criteria

| AC-ID | Given / When | Then: 관찰 가능한 기대값 | 검증 범주/테스트 또는 관찰 방법 |
|---|---|---|---|
| AC-01 | <조건/행동> | <정확한 결과> | unit: <test name> |
| AC-02 | <조건/행동> | <정확한 결과> | integration: <test name> |
| AC-03 | <조건/행동> | <정확한 결과> | e2e: <test name> |

## 작업/병렬화 계획

| 작업 | Owner 역할/인스턴스 | 수정 경로 | 선행 작업 | 산출물 |
|---|---|---|---|---|
| 구현 | code / <ID> | <정확한 파일> | 계약 READY | 구현 diff |
| 테스트 | test / <ID> | <정확한 파일> | 인터페이스 확정 | AC별 테스트 |
| 리뷰 | reviewer / <ID> | .artifacts/<run>/review.md | 고정 SHA 검증 | 독립 판정 |

공통 파일/lockfile owner: <ID>. 같은 파일 동시 수정 금지.

## 검증 환경과 명령

- 명령 설정: `.agents/verification.env`
- 준비: <버전/의존성 설치/환경변수/서버/DB/readiness>
- 실행: format --write 후 clean checkpoint, verifier가 `bash scripts/verify.sh`
- 테스트 수 확인: <범주별 summary/수집 수를 확인하는 방법>
- 격리/정리: <포트/DB/fixture/서버 종료>
- 수동 관찰: <필요할 경우 AC-ID별 절차와 증거>

## 질문과 가정

- 필수 질문: <없음 또는 답 없이는 구현 불가한 질문>
- 선택 가정: <선택과 이유>
- 위험/rollback: <관련 내용>

## 완료 조건

모든 AC 증거, 5단계 tool PASS, 동일 revision의 독립 reviewer PASS,
Builder != Verifier, clean tree를 orchestrator가 확인한다.
변경 시 버전을 올리고 재작업/재검증한다. 실패를 기준 약화로 해결하지 않는다.
