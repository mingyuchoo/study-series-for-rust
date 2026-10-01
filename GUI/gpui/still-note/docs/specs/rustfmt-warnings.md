# FMT-WARN: stable rustfmt 경고 제거

- Version: 1
- Status: READY
- Spec owner: /root (orchestrator/spec)
- Base revision: e7a3794e7d397b9c08648a7f89bd6e5472a64b4c
- Request: 첨부 로그의 모든 rustfmt unstable-option 경고 해결

## 목표와 범위

stable rustfmt에서 nightly 전용 설정 18개가 반복 경고를 낸다. 현재 toolchain을 유지하면서 실제 지원되는 설정만 사용하고 경고 없이 format/write/check를 완료한다. 필요 포맷 변경은 허용하되 제품 동작, 테스트 assertion, lint 강도를 유지한다. nightly 도입이나 경고 출력 필터링으로 숨기지 않는다.

## Acceptance criteria

| AC-ID | Given / When | Then | 검증 방법 |
|---|---|---|---|
| AC-01 | 설정된 toolchain으로 bash scripts/format.sh --write 실행 | exit 0, 첨부된 18종 unstable-option 경고 없음 | Builder 실행 로그 및 Verifier 독립 재실행 |
| AC-02 | 같은 revision에서 bash scripts/format.sh --check 실행 | exit 0, 경고 없음, tracked diff 없음 | verify format.log 및 Git 상태 |
| AC-03 | bash scripts/verify.sh 실행 | format/lint/unit/integration/e2e 5단계 PASS; 각 테스트 범주 실행 수 1 이상, 실패/skip 없음 | summary.tsv, 각 단계 로그 |
| AC-04 | 수정 diff 검토 | stable toolchain과 지원 설정 유지, 경고 억제/테스트 약화 없음; 제품 동작 변경 없음 | 독립 reviewer diff 검토 |

## 작업/병렬화 계획

- code owner: rustfmt.toml 및 전체 format으로 필요한 Rust 파일, 이 spec checkpoint commit 포함. 추가 설정 변경이 필요하면 orchestrator에게 알린다.
- test owner: .artifacts/verification/ 하위 검증 보고서와 로그만. 제품/설정 수정 금지.
- reviewer owner: .artifacts/verification/ 하위 독립 review 보고서만.
- orchestrator owner: 이 spec, .artifacts/verification/ 최종 gate 보고서.
- Writer 종료와 checkpoint 후 verifier 실행, 그 뒤 reviewer 실행. 동시 파일 수정 금지.

## 검증 환경과 명령

.agents/verification.env의 실제 명령 사용. Builder가 bash scripts/format.sh --write 후 checkpoint commit. 독립 verifier가 clean SHA에서 bash scripts/verify.sh 및 format --write의 멱등성/경고 여부를 확인한다. Rust 테스트 로그의 running/result에서 실행 수를 기록한다. GUI e2e는 기존 test-support 사용.

## 질문과 가정 / 완료 조건

필수 질문 없음. stable에서 무시되던 옵션 제거가 기존 유효 포맷 동작을 보존한다고 가정하되 도구와 diff로 확인한다. 동일 clean SHA의 AC 증거, 5단계 도구 PASS, 독립 reviewer PASS가 모두 있어야 최종 PASS. 실패 시 계약에 따라 최대 3회 재작업한다.
