# PS-SCRIPTS: PowerShell verification scripts

- Version: 2
- Status: READY
- Spec owner: /root (orchestrator acting as spec)
- Base revision: none; supplied workspace is not a Git repository
- Request: scripts 아래 모든 *.sh에 대응하는 *.ps1 작성

## 목표와 범위

Add native PowerShell equivalents of all seven existing scripts, preserving their
verification behavior. Retain the Bash scripts and their trusted Bash configuration.
Add a separate trusted PowerShell configuration and usage instructions. Require
PowerShell 7.4 or newer for reliable native-command failure handling; no Bash is
required for the PowerShell entry points. Dependency installation is out of scope.

## 계약과 설계 제약

- Files: scripts/_common.ps1, format.ps1, lint.ps1, unit-test.ps1,
  integration-test.ps1, e2e-test.ps1, verify.ps1.
- Configuration: .agents/verification.ps1 contains the same six command keys in
  PowerShell syntax, intentionally empty until the consuming project configures them.
- Commands execute from the template root, even when invoked from another directory.
- Missing configuration and blank commands fail with exit 2. PowerShell errors and
  failing native commands stop execution; native command exit codes are preserved.
- format defaults to --check and accepts --check/--write; invalid mode exits 2.
- verify requires Git, a committed clean root, and ignored /.artifacts/.
- verify executes format-check, lint, unit, integration, e2e sequentially, stopping
  on failure, log failure, HEAD changes, or tracked/untracked workspace changes.
- Evidence mirrors Bash: unique UTC run directory, revision.txt, summary.tsv with
  step/status/exit_code, per-step logs, result.env default FAIL and final PASS.
- Configuration is trusted executable code, not a dotenv file. Do not attempt to
  parse or silently translate arbitrary Bash configuration into PowerShell.
- Preserve original template configuration blanks; no dummy success commands.

## Acceptance criteria

| AC-ID | Given / When | Then: expected behavior | Verification |
|---|---|---|---|
| AC-01 | Enumerate scripts/*.sh | Every file has a runnable or dot-sourceable .ps1 equivalent; Bash originals preserved | unit inventory, parser, source comparison |
| AC-02 | Invoke wrappers and both format modes from another directory | Correct command key executes at root; default check; invalid mode exit 2 | integration wrapper dispatch |
| AC-03 | Missing/empty config, thrown errors, native command failure followed by success | Fail, stop subsequent commands, preserve native exit status | unit/integration error cases |
| AC-04 | Clean configured committed fixture, run verify | Ordered five steps; all logs and revision recorded; unique evidence; TOOL_GATE=PASS | e2e actual PowerShell subprocess |
| AC-05 | Missing Git checkpoint, dirty/wrong root, nonignored artifacts, failed step, changed HEAD/tree, logging failure | Reject or fail; no later steps; default FAIL evidence when evidence can be created | integration/e2e fault fixtures |
| AC-06 | Read documentation and configuration | PowerShell minimum version, syntax, usage, configuration trust and equivalent gate documented | independent review |

## 작업/ownership

- Builder: seven scripts/*.ps1, .agents/verification.ps1, README.md, .gitattributes.
- Test: tests/powershell/** and .artifacts/powershell/**; no product writes.
- Reviewer: .artifacts/powershell/review.md only; no product/test writes.
- Orchestrator: this spec and .artifacts/powershell orchestration/gate reports.
- All writers stop before Builder performs format; Test then runs final verification;
  Reviewer reviews the same fixed snapshot. No concurrent shared-file mutation.

## 검증 환경과 명령

Actual host: Windows, PowerShell 7.6.5, Git and Bash available. Tests use disposable
Git fixture directories and actual pwsh subprocesses. Each category must collect
at least one assertion-bearing test, with counts and logs recorded.

The delivered directory lacks Git metadata and the original Bash commands are
intentionally blank. Do not initialize or replace user Git state or fill template
defaults merely to pass. Builder may prepare a disposable verification snapshot,
configure actual format/lint/unit/integration/e2e commands there, run the mandated
Bash format --write, and checkpoint it. Verifier runs Bash verify and PowerShell
verify there; reports identify snapshot-specific configuration and compare delivered
product/test bytes to the checkpoint. The final workspace gate remains FAIL if the
required clean committed revision is absent; functional test success is reported
separately and never represented as full repository-contract PASS.

## 질문과 가정

- Required questions: none.
- Assume additions, preserving the existing .sh entry points.
- PowerShell 7.4+ is appropriate for this host and documented explicitly.
- No app server, browser, database, or external service is required for these CLI tools.
- Rollback: remove added files and revert the bounded documentation changes.

## 완료 조건

All AC evidence, actual format/lint/test execution, independent reviewer decision,
Builder != Verifier, fixed snapshot revision, and explicit workspace gate limitation.
Do not lower criteria to conceal unavailable Git/configuration preconditions.

Version 2: independent review identified native error-action preference as experimental
in PowerShell 7.3; require 7.4+, where it is mainstream, to guarantee AC-03.
