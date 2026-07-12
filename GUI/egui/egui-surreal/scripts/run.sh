#!/usr/bin/env bash
# Format, build, and test the egui-surreal project.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

usage() {
  cat <<'EOF'
Usage: scripts/run.sh [command]

Commands:
  all       Format, build (dev), and test (default)
  fmt       cargo fmt
  check     cargo check
  clippy    cargo clippy
  build     cargo build (dev profile)
  release   cargo build --release
  test      cargo test
  run       Format then cargo run
  help      Show this help

Examples:
  ./scripts/run.sh
  ./scripts/run.sh fmt
  ./scripts/run.sh test
  ./scripts/run.sh run
EOF
}

log() {
  printf '\n==> %s\n' "$*"
}

cmd_fmt() {
  log "Formatting with cargo fmt"
  cargo fmt --all
}

cmd_check() {
  log "Checking with cargo check"
  cargo check --all-targets
}

cmd_clippy() {
  log "Linting with cargo clippy"
  cargo clippy --all-targets -- -D warnings
}

cmd_build() {
  log "Building (dev profile)"
  cargo build
}

cmd_release() {
  log "Building (release profile)"
  cargo build --release
}

cmd_test() {
  log "Running tests"
  cargo test
}

cmd_run() {
  cmd_fmt
  log "Running application"
  cargo run
}

cmd_all() {
  cmd_fmt
  cmd_build
  cmd_test
  log "All steps completed successfully"
}

main() {
  local command="${1:-all}"

  case "$command" in
    all)
      cmd_all
      ;;
    fmt|format)
      cmd_fmt
      ;;
    check)
      cmd_check
      ;;
    clippy)
      cmd_clippy
      ;;
    build)
      cmd_build
      ;;
    release)
      cmd_release
      ;;
    test)
      cmd_test
      ;;
    run)
      cmd_run
      ;;
    help|-h|--help)
      usage
      ;;
    *)
      printf 'Unknown command: %s\n\n' "$command" >&2
      usage >&2
      exit 1
      ;;
  esac
}

main "$@"
