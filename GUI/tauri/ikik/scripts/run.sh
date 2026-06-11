#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

log() {
  printf '\n==> %s\n' "$1"
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'error: required command not found: %s\n' "$1" >&2
    exit 1
  fi
}

require_command cargo
require_command dx

if ! cargo tauri --version >/dev/null 2>&1; then
  printf 'error: required cargo subcommand not found: cargo tauri\n' >&2
  printf 'hint: install it with: cargo install tauri-cli\n' >&2
  exit 1
fi

log "Clean build artifacts"
cargo clean

log "Build workspace"
cargo build --workspace

log "Run workspace tests"
cargo test --workspace

log "Run Tauri development app"
cargo tauri dev
