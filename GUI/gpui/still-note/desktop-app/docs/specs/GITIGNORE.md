# GITIGNORE: Repository-generated and local files

- Version: 1
- Status: READY
- Spec owner: /root
- Base revision: 218ad03dbbf349ed5707ce479d9fb8f70c0f1bcc
- Request: Analyze the codebase and update .gitignore.

## Scope and constraints

This is a Rust/Cargo Windows desktop app with Bash/PowerShell verification scripts.
Keep source, Cargo.lock, configuration, assets, specifications and fixtures trackable.
Ignore actual build/verification outputs and narrowly identified local/transient files.
Do not introduce generic JSON, backup, log or editor-directory exclusions that hide
potential shared inputs. Default journal data lives outside the repository; any
in-repository journal exclusion must be narrowly scoped and justified by source.
No application, test or verification-command changes are required.

## Acceptance criteria

| AC-ID | Given / When | Expected behavior | Verification |
|---|---|---|---|
| AC-01 | Cargo build and verification produce files | Root target and .artifacts contents are ignored | git check-ignore positive probes |
| AC-02 | Local temporary and supported generated/local files appear | Justified patterns ignore them; comments explain groups | Source analysis and positive probes |
| AC-03 | Source, lockfile, config, docs and fixtures are considered | They remain trackable, including hypothetical JSON/log/backup fixtures | git check-ignore --no-index negative probes and tracked-file audit |
| AC-04 | Final checkpoint is verified | Existing five verification stages and independent review refer to identical SHA | bash scripts/verify.sh logs, test counts, AC report and review |

## Ownership and execution

- Builder: assigned code agent; writes .gitignore only; includes this spec in checkpoint.
- Verifier: assigned test agent; writes evidence only under .artifacts/gitignore/.
- Reviewer: separate reviewer agent; writes .artifacts/gitignore/review.md only.
- Orchestrator: this spec and final evidence report.
- Builder executes bash scripts/format.sh --write after all writers stop and commits
  only .gitignore and this spec. Verifier runs bash scripts/verify.sh on clean SHA.
- Existing .agents/verification.env commands remain unchanged. Read test logs for
  collected/executed test counts; each category must execute at least one test.
- No required questions. Global editor rules are omitted unless codebase evidence
  establishes that the files are local and not intended shared configuration.
- Final gate requires all ACs, all tools and independent review PASS; environment
  failures must be reported without weakening checks.
