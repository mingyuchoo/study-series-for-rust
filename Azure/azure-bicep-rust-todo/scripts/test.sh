#!/usr/bin/env bash
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/common.sh"
if (( $# > 0 )); then echo "Usage: $0" >&2; exit 1; fi
echo "Testing SolidJS + Rust application..."
(
    cd -- "$FRONTEND_DIR"
    run_pnpm install --frozen-lockfile
    run_pnpm run lint
    run_pnpm test
)
(cd -- "$BACKEND_DIR" && cargo test --locked)
echo "All tests passed!"
