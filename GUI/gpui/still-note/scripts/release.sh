#!/usr/bin/env bash
# Windows packaging uses the same native PowerShell pipeline.
set -Eeuo pipefail
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if ! command -v pwsh >/dev/null 2>&1; then
  printf 'Missing PowerShell 7.4+ (pwsh).\n' >&2
  exit 2
fi
script="$script_dir/release.ps1"
if command -v cygpath >/dev/null 2>&1; then
  script="$(cygpath -w "$script")"
fi
exec pwsh -NoProfile -File "$script" "$@"
