#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

cd "${PROJECT_ROOT}"

if [[ -t 1 ]]; then
  clear
fi

echo "==> Cleaning project"
cargo clean

echo "==> Building project"
cargo build

echo "==> Testing project"
cargo test

echo "==> Running project"
cargo run
