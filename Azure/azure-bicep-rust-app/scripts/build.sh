#!/usr/bin/env bash
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/common.sh"
case "${1:-}" in
    ""|--run) ;;
    --help|-h) echo "Usage: $0 [--run]"; exit 0 ;;
    *) echo "Usage: $0 [--run]" >&2; exit 1 ;;
esac
if (( $# > 1 )); then echo "Too many arguments" >&2; exit 1; fi
echo "Building integrated SolidJS + Rust application..."
build_frontend
(cd -- "$BACKEND_DIR" && cargo build --locked)
echo "Build completed successfully!"
if [[ "${1:-}" == "--run" ]]; then
    echo "Application: http://localhost:8000"
    echo "Swagger UI: http://localhost:8000/swagger-ui/"
    (cd -- "$BACKEND_DIR" && cargo run --locked)
fi
