#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "${ANDROID_SERIAL:-}" =~ ^emulator-[0-9]+$ ]] || { echo 'Set ANDROID_SERIAL to the isolated API 36 emulator.' >&2; exit 1; }
[[ "$(adb -s "$ANDROID_SERIAL" shell getprop ro.kernel.qemu | tr -d '\r')" == 1 ]]
[[ "$(adb -s "$ANDROID_SERIAL" shell getprop ro.build.version.sdk | tr -d '\r')" == 36 ]]
bash ./kotlin run -m tooling -- format
bash ./kotlin run -m tooling -- architecture
bash ./kotlin build
bash ./kotlin test
cargo run --locked --manifest-path scripts/rust-fixture/Cargo.toml -- validate build/compatibility-roundtrip.json
bash ./kotlin run -m tooling -- lint
bash ./kotlin run -m tooling -- ui
