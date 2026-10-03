#!/usr/bin/env bash
# Tool gate only: independent review + AC evidence are required for final PASS.
set -Eeuo pipefail
source "$(dirname -- "${BASH_SOURCE[0]}")/_common.sh"
command -v git >/dev/null || { printf 'FAIL: git required\n' >&2; exit 2; }
git rev-parse --verify HEAD >/dev/null 2>&1 || {
  printf 'FAIL: committed Git checkpoint required\n' >&2; exit 2;
}
git_root="$(cd -- "$(git rev-parse --show-toplevel)" && pwd)"
if [[ "$git_root" != "$REPO_ROOT" ]]; then
  project_prefix="$(git rev-parse --show-prefix)"
  git -C "$git_root" --literal-pathspecs ls-files --error-unmatch -- "${project_prefix}Cargo.toml" >/dev/null 2>&1 || {
    printf 'FAIL: nested project requires a tracked Cargo.toml in this Git repository\n' >&2; exit 2;
  }
fi
[[ -z "$(git status --porcelain --untracked-files=all)" ]] || {
  printf 'FAIL: commit changes before verification (clean tree required)\n' >&2; exit 2;
}
# Artifacts must be ignored; otherwise writing logs would invalidate the checkpoint.
git check-ignore -q .artifacts/verification/probe || {
  printf 'FAIL: add /.artifacts/ to repository .gitignore\n' >&2; exit 2;
}
revision="$(git rev-parse HEAD)"
mkdir -p -- "$REPO_ROOT/.artifacts/verification"
run_dir="$(mktemp -d "$REPO_ROOT/.artifacts/verification/$(date -u +%Y%m%dT%H%M%SZ)-XXXXXX")"
printf '%s\n' "$revision" > "$run_dir/revision.txt"
printf 'step\tstatus\texit_code\n' > "$run_dir/summary.tsv"
printf 'TOOL_GATE=FAIL\n' > "$run_dir/result.env"
printf 'Evidence: %s\n' "$run_dir"

stable_revision() {
  [[ "$(git rev-parse HEAD)" == "$revision" && -z "$(git status --porcelain --untracked-files=all)" ]]
}
run_step() {
  local step="$1" script="$2" rc
  shift 2
  if ! stable_revision; then
    printf '%s\tFAIL_REVISION_CHANGED\t1\n' "$step" >> "$run_dir/summary.tsv"
    printf 'FAIL: revision or working tree changed\n' >&2
    exit 1
  fi
  # Invoke outside an `if` so Bash errexit semantics inside child scripts stay intact.
  set +e
  bash "$SCRIPT_DIR/$script" "$@" 2>&1 | tee "$run_dir/$step.log"
  rc=$?  # pipefail preserves script failures AND logging failures.
  set -e
  if (( rc != 0 )); then
    printf '%s\tFAIL\t%s\n' "$step" "$rc" >> "$run_dir/summary.tsv"
    printf 'FAIL: %s (exit %s). Remaining steps not run.\n' "$step" "$rc" >&2
    exit "$rc"
  fi
  if ! stable_revision; then
    printf '%s\tFAIL_REVISION_CHANGED\t1\n' "$step" >> "$run_dir/summary.tsv"
    printf 'FAIL: %s modified revision or working tree\n' "$step" >&2
    exit 1
  fi
  printf '%s\tPASS\t0\n' "$step" >> "$run_dir/summary.tsv"
}
run_step format format.sh --check
run_step lint lint.sh
run_step unit unit-test.sh
run_step integration integration-test.sh
run_step e2e e2e-test.sh
printf 'TOOL_GATE=PASS\n' > "$run_dir/result.env"
printf 'TOOL PASS: %s. Final gate still requires AC evidence and independent review.\n' "$revision"
