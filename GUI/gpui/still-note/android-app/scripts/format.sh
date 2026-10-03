#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "${1:-}" == "--write" ]]; then bash ./kotlin run -m tooling -- format --write; else bash ./kotlin run -m tooling -- format; fi
