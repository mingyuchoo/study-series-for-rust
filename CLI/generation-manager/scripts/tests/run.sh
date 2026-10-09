#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
mkdir -p "$fixture/project with spaces/scripts" "$fixture/bin"
cp -- "$script_dir/run.sh" "$fixture/project with spaces/scripts/run.sh"
export CARGO_LOG="$fixture/cargo.log"
export EXPECTED_ROOT="$fixture/project with spaces"
export PATH="$fixture/bin:$PATH"

cat > "$fixture/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "$PWD" == "$EXPECTED_ROOT" ]]
printf '<%s>' "$@" >> "$CARGO_LOG"
printf '\n' >> "$CARGO_LOG"
if [[ "$1" == "${FAIL_STAGE:-}" ]]; then
    exit 42
fi
EOF
chmod +x "$fixture/bin/cargo"

cd -- "$fixture"
bash "$EXPECTED_ROOT/scripts/run.sh" > /dev/null
expected=$'<fmt><--all>\n<clippy><--workspace><--all-targets><--><-D><warnings>\n<build><--workspace>\n<test><--workspace>\n<run><--package><gm-cli><--bin><gm><--><--help>'
[[ "$(cat "$CARGO_LOG")" == "$expected" ]]

: > "$CARGO_LOG"
bash "$EXPECTED_ROOT/scripts/run.sh" -C 'a directory' project status > /dev/null
[[ "$(tail -n 1 "$CARGO_LOG")" == '<run><--package><gm-cli><--bin><gm><--><-C><a directory><project><status>' ]]

for stage in fmt clippy build test run; do
    : > "$CARGO_LOG"
    status=0
    FAIL_STAGE="$stage" bash "$EXPECTED_ROOT/scripts/run.sh" > /dev/null || status=$?
    [[ "$status" == 42 ]]
    [[ "$(tail -n 1 "$CARGO_LOG")" == "<$stage>"* ]]
done

echo 'run.sh checks passed'
