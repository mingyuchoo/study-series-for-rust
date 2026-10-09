#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

if (( $# == 0 )); then
    set -- --help
fi

echo '==> Formatting'
cargo fmt --all

echo '==> Linting'
cargo clippy --workspace --all-targets -- -D warnings

echo '==> Building'
cargo build --workspace

echo '==> Testing'
cargo test --workspace

echo '==> Running gm'
cargo run --package gm-cli --bin gm -- "$@"
