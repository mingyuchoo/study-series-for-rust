#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "${1:-}" == "--write" ]]; then bash ./gradlew spotlessApply; else bash ./gradlew spotlessCheck; fi
