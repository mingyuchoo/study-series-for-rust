#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${PROJECT_ROOT}"

if [[ "$(uname -s)" == "Darwin" ]] && ! xcrun -sdk macosx metal -v >/dev/null 2>&1; then
  cat <<'EOF'
error: Metal Toolchain is not installed.

GPUI needs Apple's Metal shader compiler to build on macOS.
Install the missing component, then run this script again:

  xcodebuild -downloadComponent MetalToolchain

If Xcode asks for license acceptance first, run:

  sudo xcodebuild -license accept
EOF
  exit 1
fi

if [[ -t 1 ]] && command -v clear >/dev/null 2>&1; then
  clear
fi

echo "==> Cleaning project"
cargo clean

echo "==> Building project"
cargo build --workspace

echo "==> Testing project"
cargo test --workspace

echo "==> Running window-app"
cargo run -p window-app -- "$@"
