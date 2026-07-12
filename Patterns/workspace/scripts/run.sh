#!/usr/bin/env bash
# Upgrade workspace crates, then format, build, and test.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

step() {
  printf '\n==> %s\n' "$*"
}

step "Upgrade crate dependency versions (Cargo.toml)"
if command -v cargo-upgrade >/dev/null 2>&1 || cargo upgrade --help >/dev/null 2>&1; then
  # Allow SemVer-incompatible bumps so versions track crates.io latest.
  cargo upgrade --incompatible allow --recursive true
else
  echo "cargo-upgrade not found; skipping Cargo.toml version bumps."
  echo "Install with: cargo install cargo-edit"
fi

step "Update lockfile to latest compatible versions (Cargo.lock)"
cargo update

step "Format"
cargo fmt --all

step "Build"
cargo build --workspace --all-targets

step "Test"
cargo test --workspace --all-targets

printf '\nAll steps completed successfully.\n'
