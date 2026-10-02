# WINDOW-MOVE-01: Restore Windows titlebar dragging

- Version: 1
- Status: READY
- Spec owner: /root
- Base revision: 0f824c31db9bf949a3b0645b392081278f959652
- Request: 빌드한 앱의 Windows 창이 움직이지 않는 결함 수정.

## Contract

Keep the existing integrated titlebar and GPUI native window controls. Restore
Windows movement through the blank titlebar region using the existing GPUI API,
without new dependencies or journal/storage changes. Preserve interactive
navigation, controls, minimum sizing, resize and maximize/restore behavior.

| AC-ID | Given / When | Expected | Verification |
|---|---|---|---|
| AC-01 | Windows production window, drag blank titlebar | Window position changes; the backend receives a caption hit | Runnable regression of rendered hit-test routing; native executable drag with before/after screenshot origins |
| AC-02 | Navigation/buttons/input or window controls receive pointer | Interactive areas retain client/control hits rather than caption; maximize/restore still work | Rendered hit-test regression, existing chrome tests; native double-click maximize/restore observation |
| AC-03 | Full repository verification and executable build | All five verification stages pass with nonzero tests in every category; application builds | Clean checkpoint `bash scripts/verify.sh`, `cargo build --locked`, logs and counts |

## Owners and sequence

- Builder /root/builder: src/ui.rs. Request ownership expansion if necessary.
- Test /root/verifier: tests/bujo_unit.rs, tests/bujo_integration.rs,
  tests/bujo_e2e.rs and .artifacts/window-move-01/**. Product code read only.
- Reviewer /root/reviewer: .artifacts/window-move-01/review.md only.
- Orchestrator /root: this spec and final gate report. No shared writer files.
- All writers finish before builder runs Git Bash `scripts/format.sh --write`
  and commits the exact task paths including spec and tests.
- Verifier uses the fixed clean SHA and runs Git Bash verification sequentially.
  Native checks use a separate temporary journal and only the isolated test app.
- Reviewer reads the same SHA diff and original verification/AC evidence.

## Environment, questions and completion

Windows, GPUI pinned 0.2.2, commands in `.agents/verification.env`.
Use installed Git Bash if the `bash` shim targets WSL. Test counts come from
the Cargo test summaries; no skipped/empty suites or assertion weakening.
No required questions. Movement means dragging the existing blank integrated
titlebar. Any unavailable native observation is a reported blocker, not PASS.
Rollback is reverting the checkpoint. Maximum three rework cycles.
Final PASS requires all ACs, five tool stages, independent reviewer PASS,
identical evidence/final SHA, and a clean tree; no merge/deployment implied.
