#!/usr/bin/env bash
# tauri-leptos-app: 포맷 + 빌드 + 테스트 + 앱 실행
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
TAURI_DIR="${PROJECT_ROOT}/tauri-entrypoint"

cd "${PROJECT_ROOT}"

if [[ -t 1 ]]; then
  RED='\033[0;31m'
  GREEN='\033[0;32m'
  YELLOW='\033[1;33m'
  BLUE='\033[0;34m'
  NC='\033[0m'
else
  RED='' GREEN='' YELLOW='' BLUE='' NC=''
fi

log_info()    { printf '%bℹ️  %s%b\n' "${BLUE}" "$1" "${NC}"; }
log_success() { printf '%b✅ %s%b\n' "${GREEN}" "$1" "${NC}"; }
log_warn()    { printf '%b⚠️  %s%b\n' "${YELLOW}" "$1" "${NC}"; }
log_error()   { printf '%b❌ %s%b\n' "${RED}" "$1" "${NC}" >&2; }
step()        { printf '\n%b==> %s%b\n' "${BLUE}" "$1" "${NC}"; }

usage() {
  cat <<EOF
Usage: scripts/run.sh [command...]

Commands:
  fmt | format   cargo fmt --all (TOML 은 taplo 가 있으면 함께 포맷)
  build          cargo build --workspace
  test           cargo test --workspace
  run | dev      Tauri 앱 개발 모드 실행 (cargo tauri dev)
  all            format + build + test (기본값)
  help           이 도움말

Examples:
  ./scripts/run.sh
  ./scripts/run.sh all
  ./scripts/run.sh fmt build test
  ./scripts/run.sh test
  ./scripts/run.sh run
  ./scripts/run.sh dev
EOF
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    log_error "필수 명령을 찾을 수 없습니다: $1"
    if [[ -n "${2:-}" ]]; then
      log_info "설치: $2"
    fi
    exit 1
  fi
}

require_cargo_subcommand() {
  # e.g. require_cargo_subcommand tauri "cargo install tauri-cli"
  local sub="$1"
  local install_hint="${2:-}"
  require_command cargo
  if ! cargo "${sub}" --version >/dev/null 2>&1 && ! command -v "cargo-${sub}" >/dev/null 2>&1; then
    log_error "필수 cargo 서브명령을 찾을 수 없습니다: cargo ${sub}"
    if [[ -n "${install_hint}" ]]; then
      log_info "설치: ${install_hint}"
    fi
    exit 1
  fi
}

ensure_wasm_target() {
  if ! rustup target list --installed 2>/dev/null | grep -qx 'wasm32-unknown-unknown'; then
    step "wasm32-unknown-unknown 타겟 추가"
    require_command rustup
    rustup target add wasm32-unknown-unknown
    log_success "wasm32-unknown-unknown 타겟 추가 완료"
  fi
}

format_code() {
  step "코드 포맷팅 (cargo fmt --all)"
  require_command cargo
  cargo fmt --all
  log_success "Rust 포맷팅 완료"

  if command -v taplo >/dev/null 2>&1; then
    step "TOML 포맷팅 (taplo format, target/dist 제외)"
    # .taplo.toml 의 include/exclude 를 따름 (target, dist 제외)
    taplo format
    log_success "TOML 포맷팅 완료"
  else
    log_info "taplo 가 없어 TOML 포맷은 건너뜁니다 (선택: cargo install taplo-cli)"
  fi
}

build_code() {
  step "워크스페이스 빌드 (cargo build --workspace)"
  require_command cargo
  cargo build --workspace
  log_success "빌드 완료"
}

run_tests() {
  step "워크스페이스 테스트 (cargo test --workspace)"
  require_command cargo
  cargo test --workspace
  log_success "테스트 완료"
}

run_app() {
  step "앱 실행 (cargo tauri dev)"
  require_command cargo
  require_cargo_subcommand tauri "cargo install tauri-cli --locked"
  require_command trunk "cargo install trunk --locked"
  ensure_wasm_target

  if [[ ! -d "${TAURI_DIR}" ]]; then
    log_error "Tauri 진입점 디렉터리를 찾을 수 없습니다: ${TAURI_DIR}"
    exit 1
  fi
  if [[ ! -f "${TAURI_DIR}/tauri.conf.json" ]]; then
    log_error "tauri.conf.json 을 찾을 수 없습니다: ${TAURI_DIR}/tauri.conf.json"
    exit 1
  fi

  log_info "작업 디렉터리: ${TAURI_DIR}"
  log_info "프론트엔드(Trunk) 는 tauri beforeDevCommand 로 함께 기동됩니다"
  log_info "종료: Ctrl+C"

  cd "${TAURI_DIR}"
  # trunk 0.21 은 NO_COLOR=1 을 bool 옵션 값으로 파싱하다 실패할 수 있음
  # (clap: invalid value '1' for '--no-color' [possible values: true, false])
  unset NO_COLOR || true
  # exec 로 교체해 시그널(Ctrl+C)이 cargo tauri 에 바로 전달되게 함
  exec cargo tauri dev
}

run_all() {
  format_code
  build_code
  run_tests
  printf '\n'
  log_success "전체 파이프라인 완료 (format + build + test)"
}

main() {
  if [[ "$#" -eq 0 ]]; then
    run_all
    return
  fi

  for cmd in "$@"; do
    case "${cmd}" in
      help|-h|--help)
        usage
        ;;
      fmt|format)
        format_code
        ;;
      build)
        build_code
        ;;
      test)
        run_tests
        ;;
      run|dev)
        run_app
        ;;
      all)
        run_all
        ;;
      *)
        log_error "알 수 없는 명령: ${cmd}"
        usage
        exit 1
        ;;
    esac
  done
}

main "$@"
