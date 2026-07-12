#!/usr/bin/env bash
# scripts/run.sh — format, build, and test the workspace.
#
# Usage:
#   ./scripts/run.sh              # format → build → test (default)
#   ./scripts/run.sh format       # cargo fmt only
#   ./scripts/run.sh build        # cargo build only
#   ./scripts/run.sh test         # cargo test only
#   ./scripts/run.sh all          # same as default
#   ./scripts/run.sh help         # show usage
#
# Options (env):
#   RELEASE=1                     # use --release for build/test
#   VERBOSE=1                     # pass --verbose to cargo

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Colors (disabled when not a TTY or NO_COLOR is set)
if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
  RED=$'\033[0;31m'
  GREEN=$'\033[0;32m'
  YELLOW=$'\033[0;33m'
  BLUE=$'\033[0;34m'
  BOLD=$'\033[1m'
  RESET=$'\033[0m'
else
  RED="" GREEN="" YELLOW="" BLUE="" BOLD="" RESET=""
fi

log()  { printf '%s==>%s %s\n' "${BLUE}${BOLD}" "${RESET}" "$*"; }
ok()   { printf '%s✓%s  %s\n' "${GREEN}${BOLD}" "${RESET}" "$*"; }
fail() { printf '%s✗%s  %s\n' "${RED}${BOLD}" "${RESET}" "$*" >&2; }
die()  { fail "$*"; exit 1; }

usage() {
  cat <<'USAGE'
Usage: ./scripts/run.sh [command]

Commands:
  all (default)   Format, build, then test the workspace
  format          Run cargo fmt --all
  build           Run cargo build --workspace
  test            Run cargo test --workspace
  help            Show this help

Environment:
  RELEASE=1       Build/test with --release
  VERBOSE=1       Pass --verbose to cargo
  NO_COLOR=1      Disable colored output
USAGE
}

require_cargo() {
  command -v cargo >/dev/null 2>&1 || die "cargo not found. Install Rust from https://rustup.rs/"
}

cargo_flags=()
[[ -n "${VERBOSE:-}" ]] && cargo_flags+=(--verbose)

release_flags=()
[[ -n "${RELEASE:-}" ]] && release_flags+=(--release)

run_format() {
  log "Formatting (cargo fmt --all)"
  cargo fmt --all
  ok "Format complete"
}

run_build() {
  log "Building (cargo build --workspace ${release_flags[*]:-})"
  cargo build --workspace "${release_flags[@]}" "${cargo_flags[@]}"
  ok "Build complete"
}

run_test() {
  log "Testing (cargo test --workspace ${release_flags[*]:-})"
  cargo test --workspace "${release_flags[@]}" "${cargo_flags[@]}"
  ok "Tests complete"
}

run_all() {
  local start
  start="$(date +%s)"

  printf '\n%s%s File Converter — format / build / test%s\n\n' "${BOLD}" "${YELLOW}" "${RESET}"
  require_cargo

  run_format
  echo
  run_build
  echo
  run_test

  local elapsed=$(( $(date +%s) - start ))
  printf '\n%sAll steps succeeded%s (%ss)\n' "${GREEN}${BOLD}" "${RESET}" "${elapsed}"
}

main() {
  local cmd="${1:-all}"

  case "$cmd" in
    all|"")
      run_all
      ;;
    format|fmt)
      require_cargo
      run_format
      ;;
    build)
      require_cargo
      run_build
      ;;
    test)
      require_cargo
      run_test
      ;;
    help|-h|--help)
      usage
      ;;
    *)
      fail "Unknown command: $cmd"
      echo
      usage
      exit 1
      ;;
  esac
}

main "$@"
