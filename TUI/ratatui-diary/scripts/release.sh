#!/usr/bin/env bash

set -Eeuo pipefail

readonly bin_name='ratatui-diary'
readonly package_name='ratatui-diary'

usage() {
    cat <<'EOF'
Usage: ./scripts/release.sh [options]

Verify and build ratatui-diary, then create a portable tar.gz archive.

Options:
      --skip-test       Skip fmt, clippy, and test verification
      --skip-build      Package an existing release binary
      --version VERSION Override the package version (default: Cargo metadata)
      --target TRIPLE   Build for a Rust target triple (default: host)
      --out-dir PATH    Output directory (default: <project>/dist)
  -h, --help            Show this help

Output:
  dist/ratatui-diary-<version>-<target>.tar.gz
EOF
}

die() {
    printf '\n%s!! %s%s\n' "$red" "$1" "$reset" >&2
    exit 1
}

write_step() {
    printf '\n%s==> %s%s\n' "$cyan" "$1" "$reset"
}

write_done() {
    printf '%s==> %s%s\n' "$green" "$1" "$reset"
}

invoke() {
    local name="$1"
    shift

    printf '\n%s==> %s :' "$cyan" "$name"
    printf ' %q' "$@"
    printf '%s\n' "$reset"

    if "$@"; then
        write_done "$name complete"
    else
        local status=$?
        die "$name failed (exit code: $status)"
    fi
}

skip_test=false
skip_build=false
version=''
target=''
out_dir=''
target_was_set=false

while (($# > 0)); do
    case "$1" in
        --skip-test)
            skip_test=true
            shift
            ;;
        --skip-build)
            skip_build=true
            shift
            ;;
        --version|--target|--out-dir)
            option="$1"
            if (($# < 2)) || [[ -z "$2" ]]; then
                printf '!! %s requires a value.\n\n' "$option" >&2
                usage >&2
                exit 2
            fi
            case "$option" in
                --version) version="$2" ;;
                --target)
                    target="$2"
                    target_was_set=true
                    ;;
                --out-dir) out_dir="$2" ;;
            esac
            shift 2
            ;;
        --version=*|--target=*|--out-dir=*)
            option="${1%%=*}"
            value="${1#*=}"
            if [[ -z "$value" ]]; then
                printf '!! %s requires a value.\n\n' "$option" >&2
                usage >&2
                exit 2
            fi
            case "$option" in
                --version) version="$value" ;;
                --target)
                    target="$value"
                    target_was_set=true
                    ;;
                --out-dir) out_dir="$value" ;;
            esac
            shift
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

for tool in cargo rustc python3 tar; do
    command -v "$tool" >/dev/null 2>&1 || \
        die "$tool was not found. Install it and try again."
done

host_triple="$(rustc -vV | sed -n 's/^host:[[:space:]]*//p')"
[[ -n "$host_triple" ]] || die 'Could not determine the host target from rustc -vV.'

if [[ -z "$target" ]]; then
    target="$host_triple"
fi
[[ "$target" =~ ^[A-Za-z0-9_.-]+$ ]] || die "Invalid target triple: $target"

if ! metadata_json="$(cargo metadata --no-deps --format-version 1)"; then
    die 'cargo metadata failed.'
fi

if ! metadata_values="$(
    printf '%s' "$metadata_json" | python3 -c '
import json
import sys

metadata = json.load(sys.stdin)
package = next(
    (item for item in metadata["packages"] if item["name"] == sys.argv[1]),
    None,
)
if package is None:
    raise SystemExit(f"Package not found in workspace: {sys.argv[1]}")
print(package["version"])
print(metadata["target_directory"])
' "$package_name"
)"; then
    die 'Could not read package information from Cargo metadata.'
fi

[[ "$metadata_values" == *$'\n'* ]] || die 'Cargo metadata returned incomplete package information.'
metadata_version="${metadata_values%%$'\n'*}"
target_dir="${metadata_values#*$'\n'}"

if [[ -z "$version" ]]; then
    version="$metadata_version"
fi
[[ "$version" =~ ^[0-9A-Za-z][0-9A-Za-z.+-]*$ ]] || die "Invalid package version: $version"

if [[ -z "$out_dir" ]]; then
    out_dir="$project_root/dist"
elif [[ "$out_dir" != /* ]]; then
    out_dir="$project_root/$out_dir"
fi

out_base="$package_name-$version-$target"
staging_dir="$out_dir/staging"
archive_root="$staging_dir/$out_base"
archive_path="$out_dir/$out_base.tar.gz"

printf '\n  Package : %s %s\n' "$package_name" "$version"
printf '  Target  : %s\n' "$target"
printf '  Output  : %s\n' "$archive_path"

if [[ "$skip_test" == false ]]; then
    printf '\n%s==> Verify : cargo fmt --all -- --check%s\n' "$cyan" "$reset"
    if ! cargo fmt --all -- --check; then
        die "Formatting check failed. Run 'cargo fmt --all' and try again."
    fi
    write_done 'Format check complete'

    invoke 'Clippy' cargo clippy --workspace --all-targets
    invoke 'Test' cargo test --workspace
else
    printf '\n%s==> Skipping Verify (--skip-test)%s\n' "$yellow" "$reset"
fi

build_args=(build --release -p "$package_name")
if [[ "$target_was_set" == true ]]; then
    build_args+=(--target "$target")
    bin_path="$target_dir/$target/release/$bin_name"
else
    bin_path="$target_dir/release/$bin_name"
fi

if [[ "$target" == *-windows-* ]]; then
    bin_path+='.exe'
fi

if [[ "$skip_build" == false ]]; then
    invoke 'Build' cargo "${build_args[@]}"
else
    printf '\n%s==> Skipping Build (--skip-build)%s\n' "$yellow" "$reset"
fi

[[ -f "$bin_path" ]] || die "Release binary was not found: $bin_path"

write_step "Stage : $staging_dir"
case "$staging_dir" in
    ''|/|./|../) die "Unsafe staging directory: $staging_dir" ;;
esac
if [[ -e "$staging_dir" ]]; then
    rm -rf -- "$staging_dir"
fi
mkdir -p -- "$archive_root"
cp -- "$bin_path" "$archive_root/"

for doc in README.md LICENSE LICENSE.md LICENSE.txt; do
    if [[ -f "$project_root/$doc" ]]; then
        cp -- "$project_root/$doc" "$archive_root/"
    fi
done

for staged_file in "$archive_root"/*; do
    [[ -f "$staged_file" ]] || continue
    file_size="$(wc -c < "$staged_file")"
    printf '    + %s (%s bytes)\n' "$(basename -- "$staged_file")" "$file_size"
done
write_done 'Stage complete'

mkdir -p -- "$out_dir"
invoke 'Package' tar -C "$staging_dir" -czf "$archive_path" "$out_base"

[[ -f "$archive_path" ]] || die "Release archive was not created: $archive_path"
archive_size="$(wc -c < "$archive_path")"

printf '\n%sRelease archive created%s\n' "$green" "$reset"
printf '  %s\n' "$archive_path"
printf '  %s bytes\n\n' "$archive_size"
printf 'Extract it with: tar -xzf %q\n' "$archive_path"
