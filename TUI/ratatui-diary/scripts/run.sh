#!/usr/bin/env bash

set -Eeuo pipefail

usage() {
    cat <<'EOF'
Usage: ./scripts/run.sh [options]

Build, test, and run the ratatui-diary workspace in order.

Options:
  -r, --release    Build and run with the release profile
      --skip-test  Skip the test step
      --no-run     Skip running the application
  -h, --help       Show this help
EOF
}

release=false
skip_test=false
no_run=false

while (($# > 0)); do
    case "$1" in
        -r|--release)
            release=true
            ;;
        --skip-test)
            skip_test=true
            ;;
        --no-run)
            no_run=true
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            printf '!! Unknown option: %s\n\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
    shift
done

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(dirname -- "$script_dir")"
cd -- "$project_root"

if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
    cyan=$'\033[36m'
    green=$'\033[32m'
    yellow=$'\033[33m'
    red=$'\033[31m'
    reset=$'\033[0m'
else
    cyan=''
    green=''
    yellow=''
    red=''
    reset=''
fi

step() {
    local name="$1"
    shift

    printf '\n%s==> %s : cargo' "$cyan" "$name"
    printf ' %q' "$@"
    printf '%s\n' "$reset"

    if cargo "$@"; then
        printf '%s==> %s complete%s\n' "$green" "$name" "$reset"
    else
        local status=$?
        printf '%s!! %s failed (exit code: %d)%s\n' "$red" "$name" "$status" "$reset" >&2
        return "$status"
    fi
}

if ! command -v cargo >/dev/null 2>&1; then
    printf '%s!! cargo was not found. Install the Rust toolchain: https://rustup.rs%s\n' \
        "$red" "$reset" >&2
    exit 1
fi

profile_args=()
if [[ "$release" == true ]]; then
    profile_args+=(--release)
fi

step 'Build' build --workspace "${profile_args[@]}"

if [[ "$skip_test" == false ]]; then
    step 'Test' test --workspace "${profile_args[@]}"
else
    printf '\n%s==> Skipping Test (--skip-test)%s\n' "$yellow" "$reset"
fi

if [[ "$no_run" == false ]]; then
    step 'Run' run "${profile_args[@]}"
else
    printf '\n%s==> Skipping Run (--no-run)%s\n' "$yellow" "$reset"
fi
