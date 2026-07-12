#!/usr/bin/env bash
# 모든 출력/주석은 한국어
set -e

# 색상
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info()    { echo -e "${BLUE}ℹ️  $1${NC}"; }
log_success() { echo -e "${GREEN}✅ $1${NC}"; }
log_warn()    { echo -e "${YELLOW}⚠️  $1${NC}"; }
log_error()   { echo -e "${RED}❌ $1${NC}"; }

# 러스트 설치 확인
check_rust() {
  if ! command -v rustc >/dev/null 2>&1; then
    log_warn "Rust가 설치되어 있지 않습니다. 설치를 진행합니다."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck disable=SC1090
    source "$HOME/.cargo/env"
  fi
  log_success "Rust 버전: $(rustc --version)"
}

# 도커 설치 확인
check_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    log_error "Docker가 설치되어 있지 않습니다. 먼저 설치해주세요."
    exit 1
  fi
  log_success "Docker 버전: $(docker --version)"
}

# 카고 빌드
cargo_build() {
  log_info "Rust 프로젝트 빌드(Release) 수행"
  cargo build --release
  log_success "빌드 완료: target/release/caching_with_redis"
}

# 도커 단독 실행(로컬 Redis 사용 시 host.docker.internal 권장)
docker_run_once() {
  local redis_url=${REDIS_URL:-"redis://host.docker.internal:6379"}
  log_info "단일 컨테이너 실행(REDIS_URL=${redis_url})"
  docker run --rm \
    -e REDIS_URL="${redis_url}" \
    -e CACHE_TTL="${CACHE_TTL:-3600}" \
    -e RUST_LOG="${RUST_LOG:-info}" \
    caching_with_redis:latest status
}

# 도커 이미지 빌드
docker_build_image() {
  log_info "Docker 이미지 빌드"
  docker build -t caching_with_redis:latest .
  log_success "이미지 빌드 완료: caching_with_redis:latest"
}

# 컴포즈로 기동
docker_compose_up() {
  log_info "docker compose up -d --build"
  docker compose up -d --build
  log_success "Compose 기동 완료"
}

# 컴포즈 종료/정리
docker_compose_down() {
  log_info "docker compose down -v"
  docker compose down -v || true
  log_success "Compose 정리 완료"
}

# 상태/테스트/통계/정리 실행 래퍼
docker_compose_cmd() {
  local subcmd="$1"; shift || true
  docker compose run --rm embedding-cache "${subcmd}" "$@"
}

print_help() {
  cat <<EOF
🦀 caching_with_redis 설치/실행 스크립트
사용법: $(basename "$0") <명령>

명령:
  check           Rust/Docker 설치 확인
  build           cargo build --release
  image           docker build -t caching_with_redis:latest .
  up              docker compose up -d --build
  down            docker compose down -v
  run-once        단일 컨테이너로 status 실행(REDIS_URL 환경변수 사용)
  status          docker compose run --rm embedding-cache status
  test [text]     docker compose run --rm embedding-cache test --text "..."
  stats           docker compose run --rm embedding-cache stats
  clear           docker compose run --rm embedding-cache clear
  help            도움말 출력
EOF
}

main() {
  local cmd="${1:-help}"; shift || true
  case "$cmd" in
    check)  check_rust; check_docker ;;
    build)  cargo_build ;;
    image)  docker_build_image ;;
    up)     docker_compose_up ;;
    down)   docker_compose_down ;;
    run-once) docker_run_once ;;
    status) docker_compose_cmd status ;;
    test)   docker_compose_cmd test --text "${1:-안녕하세요!}" ;;
    stats)  docker_compose_cmd stats ;;
    clear)  docker_compose_cmd clear ;;
    help|*) print_help ;;
  esac
}

main "$@"
