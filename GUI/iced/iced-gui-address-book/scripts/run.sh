#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${PROJECT_ROOT}"

if [[ -t 1 ]] && [[ -n "${TERM:-}" ]] && [[ "${TERM}" != "dumb" ]]; then
    clear
fi

echo "==> Cleaning project"
cargo clean

echo "==> Building presentation package"
cargo build --package presentation

echo "==> Running workspace tests"
cargo test --workspace

echo "==> Starting presentation app"
exec cargo run --package presentation -- "$@"
