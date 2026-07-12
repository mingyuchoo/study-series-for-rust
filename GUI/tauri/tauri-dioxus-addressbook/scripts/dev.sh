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

require_command dx

log "Serve frontend with hot reload (UI only)"
printf 'note: Tauri IPC (invoke) is unavailable in the browser; backend CRUD will not work.\n'
printf 'note: for full-app behavior use scripts/run.sh.\n'

cd presentation_frontend
exec dx serve --platform web
