#!/usr/bin/env bash
set -Eeuo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd -- "$REPO_ROOT"

load_config() {
  local config="$REPO_ROOT/.agents/verification.env"
  if [[ ! -f "$config" ]]; then
    printf 'FAIL: missing %s\n' "$config" >&2
    return 2
  fi
  # Configuration is reviewed repository Bash code, not untrusted input.
  source "$config"
}

run_command() {
  local key="$1" cmd="${!1-}"
  if [[ -z "${cmd//[[:space:]]/}" ]]; then
    printf 'FAIL: configure %s in .agents/verification.env\n' "$key" >&2
    return 2
  fi
  printf 'Running %s\n' "$key"
  bash -Eeuo pipefail -c "$cmd"
}
