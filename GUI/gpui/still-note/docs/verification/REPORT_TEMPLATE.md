# Verification and independent review

- Feature / spec path / version:
- Revision / run directory:
- Builder instance ID(s):
- Test author / verifier instance ID(s):
- Independent reviewer instance ID:
- Environment / tool versions / timestamp:

## Tool evidence — Test agent 작성

| 단계 | 명령/로그 경로 | Exit code | 실행 테스트 수/skip 수 | 결과 |
|---|---|---|---|---|
| format check | | | 비해당 | |
| lint/typecheck | | | 비해당 | |
| unit | | | | |
| integration | | | | |
| e2e | | | | |

## AC evidence — Test agent 작성

| AC-ID | 테스트 이름/관찰 절차 | 기대값 | 실제 결과/증거 경로 | PASS/FAIL |
|---|---|---|---|---|
| AC-01 | | | | |

모든 AC 행을 추가한다. 로그 summary와 실제 수집 테스트/실행 수를 확인한다.
명령 exit 0이어도 테스트 없음/전부 skip/미검증 AC는 FAIL이다.

## Independent review — Reviewer만 작성

- Reviewed SHA / spec version:
- 직접 읽은 diff / 도구 로그 / AC 보고서:
- Builder/Test 작성에 참여하지 않았음을 확인:
- Findings: severity, file:line, AC-ID, 재현 조건, 영향, 수정 방향
- 미해결 blocker / advisory:
- Review decision: PASS / FAIL
- 근거:

## Final gate — Orchestrator만 작성

- 현재 SHA / clean working tree 확인:
- Builder != Verifier 및 reviewer 독립성 확인:
- 도구 PASS / 테스트 개수 / AC 전체 충족 / reviewer PASS 확인:
- 최종 판정: PASS / FAIL
- 남은 위험 / blocker / 다음 행동:
- 재작업 횟수 및 이전 run 경로:

양식에 PASS를 채우는 행위는 검증을 대신하지 않는다.
코드/spec/검증 설정 변경 시 기존 보고서를 보존하고 새 run에서 다시 검증한다.
