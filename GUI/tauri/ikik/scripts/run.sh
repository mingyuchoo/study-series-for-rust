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

# The Tauri backend's `generate_context!()` macro validates, at compile time,
# that `frontendDist` (../target/dx/.../web/public) exists. That path lives
# under target/ (just wiped by `cargo clean`) and is only produced by `dx
# build`. So the frontend bundle MUST be built before any `cargo build`/`test`
# touches presentation_backend, otherwise the proc-macro panics with
# "frontendDist ... but this path doesn't exist".
log "Build frontend bundle (required before compiling the Tauri backend)"
(cd presentation_frontend && dx build --release --platform web --debug-symbols false)

log "Build workspace"
cargo build --workspace

log "Run workspace tests"
cargo test --workspace

log "Run Tauri development app"
cargo tauri dev
