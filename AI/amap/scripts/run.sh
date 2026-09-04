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

# --- LLM 공급자 설정 ---
# .env(예: AZURE_OPENAI_*)이 있으면 읽습니다. 이미 설정된 환경 변수가 .env보다 우선합니다.
# 바이너리도 시작 시 .env를 직접 읽지만, 아래 검증과 mock 기본값 판단을 위해 먼저 읽습니다.
if [[ -f .env ]]; then
    set -a
    # shellcheck disable=SC1091
    source .env
    set +a
fi

# Azure OpenAI: 키가 있으면 엔드포인트를 검증하고 배포 이름과 API 버전의 기본값을 채웁니다.
#   High 이상 effort -> AZURE_OPENAI_DEPLOYMENT (gpt-5.4)
#   Medium effort    -> AZURE_OPENAI_DEPLOYMENT_MEDIUM (gpt-5.4-mini)
#   Low effort       -> AZURE_OPENAI_DEPLOYMENT_LOW (gpt-5.4-nano)
#   의미 검색 임베딩 -> AZURE_OPENAI_EMBEDDING_DEPLOYMENT (text-embedding-3-large)
if [[ -n "${AZURE_OPENAI_API_KEY:-}" ]]; then
    if [[ -z "${AZURE_OPENAI_ENDPOINT:-}" ]]; then
        printf 'AZURE_OPENAI_API_KEY가 설정되었지만 AZURE_OPENAI_ENDPOINT가 없습니다. 예: https://<리소스>.cognitiveservices.azure.com\n' >&2
        exit 1
    fi
    export AZURE_OPENAI_ENDPOINT="${AZURE_OPENAI_ENDPOINT%/}"
    export AZURE_OPENAI_API_VERSION="${AZURE_OPENAI_API_VERSION:-2024-12-01-preview}"
    export AZURE_OPENAI_DEPLOYMENT="${AZURE_OPENAI_DEPLOYMENT:-gpt-5.4}"
    export AZURE_OPENAI_DEPLOYMENT_MEDIUM="${AZURE_OPENAI_DEPLOYMENT_MEDIUM:-gpt-5.4-mini}"
    export AZURE_OPENAI_DEPLOYMENT_LOW="${AZURE_OPENAI_DEPLOYMENT_LOW:-gpt-5.4-nano}"
    export AZURE_OPENAI_EMBEDDING_DEPLOYMENT="${AZURE_OPENAI_EMBEDDING_DEPLOYMENT:-text-embedding-3-large}"
fi

export AMAP_INSECURE_DEV="${AMAP_INSECURE_DEV:-true}"

# 실제 LLM 공급자 키가 하나라도 있으면 mock 대신 실제 공급자를 사용합니다.
llm_providers=()
[[ -n "${AZURE_OPENAI_API_KEY:-}" ]] && llm_providers+=("azure")
[[ -n "${ANTHROPIC_API_KEY:-}" ]] && llm_providers+=("anthropic")
[[ -n "${OPENAI_API_KEY:-}" ]] && llm_providers+=("openai")
if (( ${#llm_providers[@]} > 0 )); then
    export AMAP_MOCK_LLM="${AMAP_MOCK_LLM:-false}"
else
    export AMAP_MOCK_LLM="${AMAP_MOCK_LLM:-true}"
fi

log_step "3/3" "AMAP 웹 서버를 시작합니다."
if [[ "${AMAP_MOCK_LLM}" == "true" || "${AMAP_MOCK_LLM}" == "1" ]]; then
    printf 'LLM: mock (실제 공급자 키가 없거나 AMAP_MOCK_LLM=true)\n'
else
    printf 'LLM 공급자: %s\n' "${llm_providers[*]}"
    if [[ -n "${AZURE_OPENAI_API_KEY:-}" ]]; then
        printf '  Azure OpenAI 엔드포인트: %s (api-version %s)\n' "${AZURE_OPENAI_ENDPOINT}" "${AZURE_OPENAI_API_VERSION}"
        printf '  배포: high=%s medium=%s low=%s embedding=%s\n' \
            "${AZURE_OPENAI_DEPLOYMENT}" "${AZURE_OPENAI_DEPLOYMENT_MEDIUM}" \
            "${AZURE_OPENAI_DEPLOYMENT_LOW}" "${AZURE_OPENAI_EMBEDDING_DEPLOYMENT}"
    fi
fi
printf '접속 주소: http://127.0.0.1:8080\n'
printf '서버를 종료하려면 Ctrl+C를 누르십시오.\n\n'

exec cargo run --release --locked --bin control-plane
