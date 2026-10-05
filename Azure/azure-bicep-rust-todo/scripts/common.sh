#!/usr/bin/env bash
set -euo pipefail
PROJECT_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
FRONTEND_DIR="$PROJECT_ROOT/src/frontend"
BACKEND_DIR="$PROJECT_ROOT/src/backend"
DOCKER_COMPOSE_FILE="$PROJECT_ROOT/src/docker/docker-compose.yml"

run_pnpm() {
    local package_manager
    package_manager="$(node -p 'require(process.argv[1]).packageManager' "$FRONTEND_DIR/package.json")"
    npx --yes "$package_manager" "$@"
}
build_frontend() {
    echo "Building SolidJS frontend..."
    (
        cd -- "$FRONTEND_DIR"
        run_pnpm install --frozen-lockfile
        run_pnpm run build:backend
    )
}
