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
  all            검사 + 포맷 + 린트 + 빌드 + 테스트 + gRPC 및 Web 서버 지속 실행 (기본값)
  web | run      gRPC 서버 백그라운드 기동 후 Web 대시보드(http://localhost:3000) 실행 (지속 실행)
  server         gRPC 서버 포그라운드 실행 ([::1]:50051)
  client         CLI 클라이언트 실행 (gRPC 서버가 이미 실행 중이어야 함)
  both           서버 백그라운드 기동 후 클라이언트 1회 테스트 실행 후 자동 종료 (CLI 테스트용)
  ci | pipeline  검사 + 포맷 + 린트 + 빌드 + 테스트 + 클라이언트 1회 실행 후 자동 종료
  test           cargo test --workspace
  clippy         cargo clippy --workspace --all-targets -- -D warnings
  fmt | format   cargo fmt --all
  build          cargo build --profile dev
  release        cargo build --profile release
  check          필수 도구 확인 (cargo, protoc)
  help           이 도움말

Examples:
  ./scripts/run.sh            # 전체 검사/빌드 후 gRPC 및 Web 대시보드 지속 실행 (권장)
  ./scripts/run.sh web        # gRPC 및 Web 대시보드 즉시 지속 실행 (http://localhost:3000)
  ./scripts/run.sh both       # 서버 기동 후 클라이언트 1회 테스트 후 종료
  ./scripts/run.sh ci         # CI용 비대화형 파이프라인 검증 후 종료
  ./scripts/run.sh server     # gRPC 서버만 단독 실행
  ./scripts/run.sh client     # 클라이언트만 단독 실행

Notes:
  - './scripts/run.sh' 또는 './scripts/run.sh web' 실행 시 서버가 종료되지 않고 계속 실행 상태를 유지합니다.
  - 브라우저에서 http://localhost:3000 에 접속하여 대시보드를 사용할 수 있습니다. (데스크톱 환경에서는 자동 열림)
  - 종료하려면 터미널에서 Ctrl+C 를 누르면 모든 서버가 안전하게 자동 종료됩니다.
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

SERVER_PID=""
WEB_PID=""
CLEANED_UP=false

is_server_ready() {
  if (cat < /dev/null > /dev/tcp/::1/50051) 2>/dev/null; then
    return 0
  elif command -v nc >/dev/null 2>&1 && nc -z -w 1 ::1 50051 2>/dev/null; then
    return 0
  fi
  return 1
}

cleanup() {
  if [[ "${CLEANED_UP}" == true ]]; then
    return 0
  fi
  CLEANED_UP=true
  trap - EXIT INT TERM

  printf '\n'
  log_info "서비스 종료 처리 중..."

  if [[ -n "${WEB_PID:-}" ]] && kill -0 "${WEB_PID}" 2>/dev/null; then
    log_info "Web 대시보드 종료 중 (PID: ${WEB_PID})..."
    pkill -P "${WEB_PID}" 2>/dev/null || true
    kill "${WEB_PID}" 2>/dev/null || true
    wait "${WEB_PID}" 2>/dev/null || true
    WEB_PID=""
  fi

  if [[ -n "${SERVER_PID:-}" ]] && kill -0 "${SERVER_PID}" 2>/dev/null; then
    log_info "gRPC 서버 종료 중 (PID: ${SERVER_PID})..."
    pkill -P "${SERVER_PID}" 2>/dev/null || true
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
    SERVER_PID=""
  fi

  log_success "모든 서버가 안전하게 종료되었습니다."
}

start_grpc_server() {
  if is_server_ready; then
    log_warn "이미 [::1]:50051 에서 gRPC 서버가 실행 중입니다. 기존 서버에 연결합니다."
    return 0
  fi

  log_info "gRPC 서버 백그라운드 기동 중..."
  cargo run -p server &
  SERVER_PID=$!
  trap cleanup EXIT INT TERM

  log_info "gRPC 서버 준비 대기 중..."
  local ready=false
  for ((i = 0; i < 30; i++)); do
    if ! kill -0 "${SERVER_PID}" 2>/dev/null; then
      log_error "gRPC 서버가 예기치 않게 종료되었습니다."
      cleanup
      return 1
    fi
    if is_server_ready; then
      ready=true
      break
    fi
    sleep 0.2
  done

  if [[ "${ready}" != true ]]; then
    log_error "gRPC 서버 기동 대기 시간 초과 (6초)"
    cleanup
    return 1
  fi
  log_success "gRPC 서버 준비 완료 (주소: [::1]:50051)"
}

open_browser() {
  local url="$1"
  if [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]; then
    if command -v xdg-open >/dev/null 2>&1; then
      (sleep 1 && xdg-open "${url}" >/dev/null 2>&1) &
    elif command -v open >/dev/null 2>&1; then
      (sleep 1 && open "${url}" >/dev/null 2>&1) &
    fi
  fi
}

run_server() {
  step "gRPC 서버 실행 (cargo run -p server)"
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

run_both() {
  step "서버 및 클라이언트 실행 (단회 실행 테스트)"
  require_command cargo

  local started_server=false

  if is_server_ready; then
    log_warn "이미 [::1]:50051 에서 서버가 실행 중입니다. 기존 서버에 연결합니다."
  else
    start_grpc_server
    started_server=true
  fi

  log_info "클라이언트 실행..."
  cargo run -p client

  if [[ "${started_server}" == true ]]; then
    cleanup
    log_success "서버 및 클라이언트 실행 테스트 완료"
  fi
}

run_web() {
  step "gRPC 서버 및 Web 대시보드 실행 (지속 실행)"
  require_command cargo

  start_grpc_server

  # gRPC 클라이언트 테스트 및 초기 샘플 데이터 등록
  log_info "gRPC 클라이언트 통신 확인 및 샘플 데이터 등록..."
  cargo run -p client || true

  printf '\n'
  printf '%b======================================================================%b\n' "${GREEN}" "${NC}"
  printf '%b🚀 e-Commerce gRPC 서비스 및 Web 대시보드가 정상 실행되었습니다!%b\n' "${GREEN}" "${NC}"
  printf '   🌐 %bWeb 대시보드:%b  http://localhost:3000\n' "${BLUE}" "${NC}"
  printf '   ⚡ %bgRPC 서버:    %b  http://[::1]:50051\n' "${BLUE}" "${NC}"
  printf '\n'
  printf '   💡 브라우저에서 %bhttp://localhost:3000%b 에 접속하여 모니터링할 수 있습니다.\n' "${YELLOW}" "${NC}"
  printf '   🛑 서비스를 종료하려면 %bCtrl+C%b 를 누르세요.\n' "${YELLOW}" "${NC}"
  printf '%b======================================================================%b\n\n' "${GREEN}" "${NC}"

  open_browser "http://localhost:3000"

  cargo run -p web || true

  cleanup
}

run_ci() {
  check_prerequisites
  format_code
  run_clippy
  build_code
  run_tests
  run_both
  printf '\n'
  log_success "전체 파이프라인 검증 완료 (format + clippy + build + test + client)"
}

run_all() {
  check_prerequisites
  format_code
  run_clippy
  build_code
  run_tests
  run_web
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
      both)
        run_both
        ;;
      web|run|serve)
        run_web
        ;;
      ci|pipeline|test-all)
        run_ci
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
