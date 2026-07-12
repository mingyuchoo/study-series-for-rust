#!/usr/bin/env bash
# axum-sqlx-postgres: PostgreSQL 이미지 pull/기동 + 포맷 + 빌드 + 테스트
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
COMPOSE_FILE="${PROJECT_ROOT}/docker/docker-compose.yaml"
POSTGRES_SERVICE="postgres"
POSTGRES_CONTAINER="axum-sqlx-postgres-db"
POSTGRES_IMAGE="postgres:17.6"
# 호스트 5433 → 컨테이너 5432 (다른 로컬 Postgres 와 충돌 방지)
DATABASE_URL_DEFAULT="postgres://postgres:postgres@localhost:5433/axum-sqlx-postgres"

# docker compose 대기 타임아웃 (초)
POSTGRES_WAIT_TIMEOUT="${POSTGRES_WAIT_TIMEOUT:-120}"

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
  pull           PostgreSQL Docker 이미지 다운로드 (${POSTGRES_IMAGE})
  postgres | db  Docker로 PostgreSQL(${POSTGRES_SERVICE}) 기동하고 healthy 대기
  down           PostgreSQL 컨테이너 중지 (볼륨 유지)
  down-v         PostgreSQL 컨테이너 중지 및 볼륨 삭제
  env            .env 가 없으면 .env.example 에서 생성
  fmt | format   cargo fmt --all
  build          cargo build --profile dev
  test           cargo test
  all            pull + env + postgres + format + build + test (기본값)
  help           이 도움말

Examples:
  ./scripts/run.sh
  ./scripts/run.sh all
  ./scripts/run.sh pull postgres fmt build test
  ./scripts/run.sh down

Environment:
  DATABASE_URL             기본값: ${DATABASE_URL_DEFAULT}
  POSTGRES_WAIT_TIMEOUT    PostgreSQL healthy 대기 초 (기본: 120)
EOF
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    log_error "필수 명령을 찾을 수 없습니다: $1"
    exit 1
  fi
}

require_docker() {
  require_command docker
  if ! docker info >/dev/null 2>&1; then
    log_error "Docker 데몬에 연결할 수 없습니다. Docker 가 실행 중인지 확인하세요."
    exit 1
  fi
}

compose() {
  # app 서비스 빌드 없이 postgres 만 제어
  docker compose -f "${COMPOSE_FILE}" --project-directory "${PROJECT_ROOT}" "$@"
}

pull_image() {
  step "Docker 이미지 다운로드 (${POSTGRES_IMAGE})"
  require_docker
  docker pull "${POSTGRES_IMAGE}"
  log_success "이미지 준비 완료: ${POSTGRES_IMAGE}"
}

ensure_env() {
  step ".env 확인"
  if [[ -f "${PROJECT_ROOT}/.env" ]]; then
    log_success ".env 파일이 이미 존재합니다"
    return
  fi

  if [[ ! -f "${PROJECT_ROOT}/.env.example" ]]; then
    log_warn ".env.example 이 없어 기본값으로 .env 를 생성합니다"
    cat > "${PROJECT_ROOT}/.env" <<EOF
DATABASE_URL=${DATABASE_URL_DEFAULT}
JWT_SECRET=dev_change_me
ACCESS_TOKEN_TTL_SECS=30
REFRESH_TOKEN_TTL_DAYS=30
RUST_LOG=axum-sqlx-postgres=debug,tower_http=debug,sqlx=warn
EOF
  else
    cp "${PROJECT_ROOT}/.env.example" "${PROJECT_ROOT}/.env"
    log_success ".env.example 을 복사해 .env 를 생성했습니다"
  fi
}

start_postgres() {
  step "Docker PostgreSQL 기동 (${POSTGRES_SERVICE})"
  require_docker

  compose up -d "${POSTGRES_SERVICE}"
  wait_for_postgres
}

wait_for_postgres() {
  step "PostgreSQL healthy 대기 (timeout: ${POSTGRES_WAIT_TIMEOUT}s)"
  local elapsed=0
  local interval=2
  local status=""

  while (( elapsed < POSTGRES_WAIT_TIMEOUT )); do
    if docker inspect -f '{{.State.Health.Status}}' "${POSTGRES_CONTAINER}" >/dev/null 2>&1; then
      status="$(docker inspect -f '{{.State.Health.Status}}' "${POSTGRES_CONTAINER}" 2>/dev/null || true)"
      if [[ "${status}" == "healthy" ]]; then
        log_success "PostgreSQL 이 준비되었습니다 (container: ${POSTGRES_CONTAINER})"
        return
      fi
      log_info "상태: ${status:-unknown} (${elapsed}s)"
    else
      # healthcheck 가 아직 없거나 컨테이너가 뜨는 중일 때
      if docker exec "${POSTGRES_CONTAINER}" pg_isready -U postgres >/dev/null 2>&1; then
        log_success "PostgreSQL 이 응답합니다 (container: ${POSTGRES_CONTAINER})"
        return
      fi
      log_info "컨테이너 준비 중... (${elapsed}s)"
    fi
    sleep "${interval}"
    elapsed=$((elapsed + interval))
  done

  log_error "PostgreSQL 이 ${POSTGRES_WAIT_TIMEOUT}초 내에 준비되지 않았습니다"
  compose ps || true
  compose logs --tail=50 "${POSTGRES_SERVICE}" || true
  exit 1
}

stop_postgres() {
  step "PostgreSQL 중지"
  require_docker
  compose stop "${POSTGRES_SERVICE}" || true
  log_success "PostgreSQL 중지 완료"
}

down_postgres() {
  local remove_volumes="${1:-false}"
  step "PostgreSQL 정리 (volumes=${remove_volumes})"
  require_docker
  if [[ "${remove_volumes}" == "true" ]]; then
    compose down -v --remove-orphans
  else
    compose down --remove-orphans
  fi
  log_success "정리 완료"
}

format_code() {
  step "코드 포맷팅 (cargo fmt --all)"
  require_command cargo
  cargo fmt --all
  log_success "포맷팅 완료"
}

build_code() {
  step "빌드 (cargo build --profile dev)"
  require_command cargo
  export DATABASE_URL="${DATABASE_URL:-${DATABASE_URL_DEFAULT}}"
  cargo build --profile dev
  log_success "빌드 완료"
}

run_tests() {
  step "테스트 (cargo test)"
  require_command cargo
  export DATABASE_URL="${DATABASE_URL:-${DATABASE_URL_DEFAULT}}"
  cargo test
  log_success "테스트 완료"
}

run_all() {
  pull_image
  ensure_env
  start_postgres
  format_code
  build_code
  run_tests
  printf '\n'
  log_success "전체 파이프라인 완료 (pull + postgres + format + build + test)"
  log_info "PostgreSQL 은 계속 실행 중입니다. 중지: ./scripts/run.sh down"
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
      pull)
        pull_image
        ;;
      postgres|db)
        start_postgres
        ;;
      down)
        down_postgres false
        ;;
      down-v)
        down_postgres true
        ;;
      env)
        ensure_env
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
