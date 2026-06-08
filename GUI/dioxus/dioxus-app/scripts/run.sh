#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PLATFORM="${DIOXUS_PLATFORM:-web}"

cd "${PROJECT_ROOT}"

usage() {
  cat <<'EOF'
Usage:
  ./scripts/run.sh [clear|clean|build|test|run|all]...

Examples:
  ./scripts/run.sh
  ./scripts/run.sh all
  ./scripts/run.sh clean build test
  DIOXUS_PLATFORM=desktop ./scripts/run.sh run
EOF
}

step() {
  local name="$1"
  shift

  printf '\n==> %s\n' "${name}"
  "$@"
}

clear_project() {
  step "Clear" cargo clean
}

build_project() {
  step "Build" cargo build
}

test_project() {
  step "Test" cargo test
}

run_project() {
  if command -v dx >/dev/null 2>&1; then
    step "Run" dx serve --platform "${PLATFORM}"
    return
  fi

  if command -v dioxus >/dev/null 2>&1; then
    step "Run" dioxus serve --platform "${PLATFORM}"
    return
  fi

  cat >&2 <<'EOF'

Error: Dioxus CLI was not found.

Install it with:
  cargo install cargo-binstall
  cargo binstall dioxus-cli

EOF
  return 127
}

run_all() {
  clear_project
  build_project
  test_project
  run_project
}

if [[ "$#" -eq 0 ]]; then
  run_all
  exit 0
fi

for command in "$@"; do
  case "${command}" in
    clear | clean)
      clear_project
      ;;
    build)
      build_project
      ;;
    test)
      test_project
      ;;
    run)
      run_project
      ;;
    all)
      run_all
      ;;
    -h | --help | help)
      usage
      ;;
    *)
      printf 'Unknown command: %s\n\n' "${command}" >&2
      usage >&2
      exit 2
      ;;
  esac
done
