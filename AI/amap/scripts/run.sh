#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

log_step() {
    printf '\n[%s] %s\n' "$1" "$2"
}

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf '필수 명령을 찾을 수 없습니다: %s\n' "$1" >&2
        exit 1
    fi
}

require_command cargo
require_command python3
require_command npm

log_step "1/7" "Rust 워크스페이스 전체 타깃을 빌드합니다."
cargo build --workspace --all-targets --locked

log_step "2/7" "Finance WASM 플러그인을 빌드합니다."
cargo build \
    --release \
    --target wasm32-unknown-unknown \
    --manifest-path "${REPO_ROOT}/plugins/finance/Cargo.toml" \
    --locked

log_step "3/7" "Rust 워크스페이스 전체 타깃을 테스트합니다."
cargo test --workspace --all-targets --locked

log_step "4/7" "Finance 플러그인을 테스트합니다."
cargo test \
    --manifest-path "${REPO_ROOT}/plugins/finance/Cargo.toml" \
    --locked

log_step "5/7" "React 웹 UI를 테스트합니다."
npm --prefix web test

log_step "6/7" "React 웹 UI의 타입, 린트, 프로덕션 빌드를 검증합니다."
npm --prefix web run lint
npm --prefix web run build

log_step "7/7" "오프라인 데모를 실행합니다."
cargo run --locked --quiet --bin amap -- demo

printf '\n빌드, 테스트, 실행을 모두 완료했습니다.\n'
