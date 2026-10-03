#!/usr/bin/env bash
set -Eeuo pipefail
source "$(dirname -- "${BASH_SOURCE[0]}")/_common.sh"
load_config
case "${1:---check}" in
  --check) run_command FORMAT_CHECK_CMD ;;
  --write) run_command FORMAT_WRITE_CMD ;;
  *) printf 'Usage: bash scripts/format.sh [--check|--write]\n' >&2; exit 2 ;;
esac
