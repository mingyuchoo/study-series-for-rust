# WIN-LINK: Windows ARM64 native dependency linking

- Version: 1
- Status: READY
- Spec owner: /root
- Base revision: 9cbb7edbd7dbe81debf35fa620864ac5adfc613f
- Request: attached build log reports libgit2 unresolved close/read/write on aarch64-pc-windows-msvc.

## Scope and constraints

Fix the Windows ARM64 build and test linking failure with the smallest justified native build/configuration change. Preserve application behavior, supported targets, and all tests/assertions. Do not edit Cargo registry sources or mask unresolved symbols. No UI/data changes. Required questions: none; the supplied failure log is treated as a request to repair the build.

## Acceptance criteria

| AC-ID | Given / When | Expected result | Verification and evidence |
|---|---|---|---|
| AC-01 | Windows ARM64 MSVC, cargo build --locked and cargo test --locked --features test-support --no-run | Application and test executables link without unresolved close/read/write | Verifier command exit codes and logs |
| AC-02 | Clean checkpoint, bash scripts/verify.sh | All five stages PASS; unit, integration, e2e each execute at least one test | summary.tsv and raw test logs |
| AC-03 | Review native fix and target boundaries | Root cause supported by source/tool evidence; fix does not alter other targets or weaken tests | Independent review and target-specific regression evidence where meaningful |

## Ownership and execution

- Builder: dedicated code agent; owns build.rs, Cargo.toml, Cargo.lock, .cargo/**, .agents/verification.env, build/run scripts and narrowly related README build documentation if necessary. Product source requires ownership approval before edits.
- Test: dedicated test agent; owns any narrowly necessary regression test under tests/ and .artifacts/windows-arm64-link/** evidence. Coordinate test additions before builder format/checkpoint.
- Reviewer: dedicated read-only reviewer; writes review evidence only under .artifacts/windows-arm64-link/**.
- Orchestrator owns this spec and final report. No concurrent writes to shared files.
- Test design may occur while builder investigates, but tool verification starts only after all writers finish and builder runs bash scripts/format.sh --write and commits a clean checkpoint.
- Verifier runs bash scripts/verify.sh at that exact SHA plus AC-01 commands sequentially in this workspace; record actual toolchain/environment and test counts. No external services are needed.
- If an environment or code failure occurs, return exact command, exit code, log and AC to builder; at most three rework cycles, each followed by full verification and new independent review.

## Completion

All AC evidence and independent reviewer PASS must refer to the final clean SHA. Report revision, spec version, actual agent IDs, execution directory, counts, evidence and remaining risks. Rollback consists of reverting the scoped checkpoint.
