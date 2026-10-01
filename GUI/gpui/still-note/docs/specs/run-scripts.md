# RUN-SCRIPTS: 전체 개발 실행 파이프라인

- Version: 2
- Status: READY
- Spec owner: /root
- Base revision: e58b72f5d5ea4406797717e6e4956b235be63d12
- Request: 전체 코드베이스 포맷팅, 린팅, 테스팅, 빌딩, 실행용 scripts/run.ps1 및 scripts/run.sh

## 목표와 범위

기존 Cargo 설정과 검증 wrapper를 사용해 개발자가 한 명령으로 포맷 수정, lint, 전체 테스트, production build, GUI 실행을 순차 수행한다. 검증용 verify 스크립트의 clean checkpoint 제약과 개발 실행은 별개다. 기존 사용자 파일 .taplo.toml, rustfmt.toml은 수정하거나 commit하지 않는다.

## 계약과 설계 제약

PowerShell 7.4 이상 및 Bash를 지원한다. Windows 네이티브 Rust/GPUI 앱이며 Bash는 Windows Git Bash 사용을 문서화한다. 호출 위치와 공백이 포함된 경로에 독립적이다. 기본은 모든 단계 후 cargo run --locked이며 --로 구분한 이후 인수를 앱에 그대로 전달한다. --no-run은 GUI 실행만 생략하고 모든 앞 단계를 수행한다. --help는 설명만 출력한다. 다른 스크립트 옵션은 오류로 종료한다. 각 단계는 실패 시 즉시 중단하고 실제 종료 코드를 보존한다. 개인 저널을 검증 중 열거나 수정하지 않는다. 전체 테스트는 --all-targets --features test-support와 별도 doctest를 포함한다. 모든 Cargo dependency resolution 명령은 --locked를 사용한다.

## Acceptance criteria

| AC-ID | Given / When | Then | 검증 방법 |
|---|---|---|---|
| AC-01 | 기본 실행, 두 셸 | format write → lint → 전체 tests 및 doctests → build → run 순서로 실행한다. 기존 환경 설정과 Cargo manifest에 일치한다. | 독립 fixture 명령 기록 및 실제 verify/build |
| AC-02 | 다른 cwd 또는 공백 경로 | 저장소 루트에서 명령 실행; -- 이후 인수 경계와 공백 보존 | 독립 fixture 테스트 |
| AC-03 | 각 단계 실패 | 후속 단계 미실행, 실패 종료 코드 보존 | 셸별 실패 주입 테스트 |
| AC-04 | --no-run, --help, 잘못된 옵션 | no-run은 앞 단계 모두 실행; help는 명령 미실행; 잘못된 옵션은 nonzero | 독립 fixture 테스트 |
| AC-05 | 실제 코드베이스 | format/lint/unit/integration/e2e 필수 도구 검증 및 production build; 테스트 수 0 불허 | clean checkpoint에서 bash scripts/verify.sh 및 전체 테스트/build 로그 |
| AC-06 | 문서와 사용자 파일 | 실행 예시, 요구 환경, 앱 인수/no-run 문서 제공; 기존 사용자 파일 보존 | diff 리뷰 및 파일 해시 비교 |

## 소유권과 산출물

- Builder: scripts/run.ps1, scripts/run.sh, README.md, .agents/verification.env, .agents/verification.ps1, scripts/verify.sh, scripts/verify.ps1.
- Test: tests/run-scripts/ 하위 fixture/tests 및 .artifacts/ 하위 증거.
- Reviewer: .artifacts/ 하위 리뷰만 작성.
- Orchestrator: 이 spec 및 판정 보고서. 공유 제품 파일 변경 없음.

## 가정과 질문

필수 질문 없음. 기본 개발 실행은 포맷을 수정한다. GUI는 종료될 때까지 foreground로 유지된다. 사용자 untracked 설정은 원래 작업 디렉터리에 보존하며 독립 clean worktree에서 checkpoint를 검증한다. GUI 수동 실행이 불가능하면 실행 자체는 fixture로 확인하고 실제 GPU 창 확인 제한을 명시한다.

## v2 조사 반영

Git 루트가 상위 study-series-for-rust 저장소이므로 기존 verify의 프로젝트 루트 동일성 검사가 실제 환경에서 실패한다. 별도 Cargo.toml이 해당 저장소에 tracked된 하위 프로젝트는 검증을 허용하고, 저장소 전체 clean 상태와 SHA 안정성 검사는 유지한다. manifest 없는 복사된 nested template는 계속 거부한다. 독립 테스트로 두 셸의 허용/거부 경계를 확인한다. LINT_CMD의 기존 build를 별도 실행 단계로 이동하여 format → lint → tests → build → run 순서를 지킨다. verify 도구 단계에는 build가 포함되지 않으므로 verifier가 production build 로그를 별도로 남긴다.
