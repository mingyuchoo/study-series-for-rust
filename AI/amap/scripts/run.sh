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
require_command npm

log_step "1/3" "React 웹 UI 의존성을 설치합니다."
if [[ -f web/package-lock.json ]]; then
    npm --prefix web ci
else
    printf 'web/package-lock.json이 없어 npm install로 대체합니다.\n'
    npm --prefix web install
fi

log_step "2/3" "React 웹 UI의 프로덕션 번들을 빌드합니다."
npm --prefix web run build

export AMAP_INSECURE_DEV="${AMAP_INSECURE_DEV:-true}"
export AMAP_MOCK_LLM="${AMAP_MOCK_LLM:-true}"

log_step "3/3" "AMAP 웹 서버를 시작합니다."
printf '접속 주소: http://127.0.0.1:8080\n'
printf '서버를 종료하려면 Ctrl+C를 누르십시오.\n\n'

exec cargo run --release --locked --bin control-plane
