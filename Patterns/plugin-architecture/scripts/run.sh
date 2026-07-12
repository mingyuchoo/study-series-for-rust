#!/usr/bin/env bash
# Format, build, and test the entire workspace.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

echo "==> Formatting (cargo fmt)"
cargo fmt --all

echo ""
echo "==> Building (cargo build)"
cargo build --workspace

echo ""
echo "==> Testing (cargo test)"
cargo test --workspace

echo ""
echo "All steps completed successfully."
