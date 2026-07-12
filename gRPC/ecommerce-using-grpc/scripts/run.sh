#!/usr/bin/env bash
# ecommerce-using-grpc: 포맷 + 빌드 + 테스트 + 서버/클라이언트 실행
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

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
  check          필수 도구 확인 (cargo, protoc)
  fmt | format   cargo fmt --all
  clippy         cargo clippy --workspace --all-targets -- -D warnings
  build          cargo build --profile dev
  release        cargo build --profile release
  test           cargo test --workspace
  server         cargo run -p server
  client         cargo run -p client
  all            format + clippy + build + test (기본값)
  help           이 도움말

Examples:
  ./scripts/run.sh
  ./scripts/run.sh all
  ./scripts/run.sh fmt build test
  ./scripts/run.sh server
  ./scripts/run.sh client

Notes:
  - 서버는 [::1]:50051 에서 대기합니다.
  - client 실행 전에 다른 터미널에서 server 를 먼저 기동하세요.
EOF
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    log_error "필수 명령을 찾을 수 없습니다: $1"
    exit 1
  fi
}

check_prerequisites() {
  step "필수 도구 확인"
  require_command cargo
  require_command rustc

  if ! command -v protoc >/dev/null 2>&1; then
    log_error "protoc(Protocol Buffers 컴파일러)를 찾을 수 없습니다."
    log_info "설치 예: sudo apt install protobuf-compiler  |  brew install protobuf"
    exit 1
  fi

  log_success "Rust: $(rustc --version)"
  log_success "Cargo: $(cargo --version)"
  log_success "protoc: $(protoc --version)"
}

format_code() {
  step "코드 포맷팅 (cargo fmt --all)"
  require_command cargo
  cargo fmt --all
  log_success "포맷팅 완료"
}

run_clippy() {
  step "Clippy (cargo clippy --workspace --all-targets)"
  require_command cargo
  cargo clippy --workspace --all-targets -- -D warnings
  log_success "Clippy 완료"
}

build_code() {
  step "빌드 (cargo build --profile dev)"
  require_command cargo
  cargo build --profile dev
  log_success "빌드 완료"
}

build_release() {
  step "릴리스 빌드 (cargo build --profile release)"
  require_command cargo
  cargo build --profile release
  log_success "릴리스 빌드 완료"
}

run_tests() {
  step "테스트 (cargo test --workspace)"
  require_command cargo
  cargo test --workspace
  log_success "테스트 완료"
}

run_server() {
  step "서버 실행 (cargo run -p server)"
  require_command cargo
  log_info "리스닝 주소: [::1]:50051"
  cargo run -p server
}

run_client() {
  step "클라이언트 실행 (cargo run -p client)"
  require_command cargo
  log_info "연결 대상: http://[::1]:50051 (서버가 먼저 실행 중이어야 합니다)"
  cargo run -p client
}

run_all() {
  check_prerequisites
  format_code
  run_clippy
  build_code
  run_tests
  printf '\n'
  log_success "전체 파이프라인 완료 (format + clippy + build + test)"
  log_info "서버 실행: ./scripts/run.sh server"
  log_info "클라이언트 실행(다른 터미널): ./scripts/run.sh client"
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
      check)
        check_prerequisites
        ;;
      fmt|format)
        format_code
        ;;
      clippy)
        run_clippy
        ;;
      build)
        build_code
        ;;
      release)
        build_release
        ;;
      test)
        run_tests
        ;;
      server)
        run_server
        ;;
      client)
        run_client
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
