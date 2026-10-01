#!/usr/bin/env bash
# Development pipeline: formatting intentionally updates source files.
set -Eeuo pipefail

usage() {
  cat <<'USAGE'
Usage: bash scripts/run.sh [--no-run] [--help] [-- APP_ARGUMENTS...]

Format, lint, test all targets and doctests, build, then run Stillnote.
  --no-run  Complete all checks and the build without opening the GUI.
  --help    Show this help without running any commands.
  --        Pass the remaining arguments unchanged to Stillnote.

Requires Rust stable with rustfmt/clippy; on Windows use Git Bash.
USAGE
}

no_run=false
app_arguments=()
while (( $# > 0 )); do
  case "$1" in
    --no-run) no_run=true; shift ;;
    --help) usage; exit 0 ;;
    --) shift; app_arguments=("$@"); break ;;
    *) printf 'Unknown script option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

source "$(dirname -- "${BASH_SOURCE[0]}")/_common.sh"

run_stage() {
  local stage="$1" rc
  shift
  printf '\n== %s ==\n' "$stage"
  set +e
  "$@"
  rc=$?
  set -e
  if (( rc != 0 )); then
    printf 'FAIL: %s (exit %s). Remaining stages not run.\n' "$stage" "$rc" >&2
    exit "$rc"
  fi
}

run_stage format bash "$SCRIPT_DIR/format.sh" --write
run_stage lint bash "$SCRIPT_DIR/lint.sh"
run_stage test cargo test --locked --all-targets --features test-support
run_stage doctest cargo test --locked --doc --features test-support
run_stage build cargo build --locked
if [[ "$no_run" == false ]]; then
  if (( ${#app_arguments[@]} > 0 )); then
    run_stage run cargo run --locked -- "${app_arguments[@]}"
  else
    run_stage run cargo run --locked
  fi
fi
