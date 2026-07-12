#!/usr/bin/env bash
# actix-sqlx-mysql: MySQL 기동 + 포맷 + 빌드 + 테스트
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
COMPOSE_FILE="${PROJECT_ROOT}/docker/docker-compose.yml"
MYSQL_SERVICE="mysql-db"
MYSQL_CONTAINER="mysql-db"
DATABASE_URL_DEFAULT="mysql://test:test@localhost:3306/test"

# docker compose 대기 타임아웃 (초)
MYSQL_WAIT_TIMEOUT="${MYSQL_WAIT_TIMEOUT:-120}"

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
  mysql | db     Docker로 MySQL(mysql-db)만 기동하고 healthy 대기
  down           MySQL 컨테이너 중지 (볼륨 유지)
  down-v         MySQL 컨테이너 중지 및 볼륨 삭제
  env            .env 가 없으면 .env.example 에서 생성
  fmt | format   cargo fmt
  build          cargo build --profile dev
  test           cargo test
  all            env + mysql + format + build + test (기본값)
  help           이 도움말

Examples:
  ./scripts/run.sh
  ./scripts/run.sh all
  ./scripts/run.sh mysql fmt build test
  ./scripts/run.sh down

Environment:
  DATABASE_URL          기본값: ${DATABASE_URL_DEFAULT}
  MYSQL_WAIT_TIMEOUT    MySQL healthy 대기 초 (기본: 120)
EOF
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    log_error "필수 명령을 찾을 수 없습니다: $1"
    exit 1
  fi
}

compose() {
  # 프로젝트 루트 기준으로 compose 파일을 지정해 app 빌드 없이 DB만 제어
  docker compose -f "${COMPOSE_FILE}" --project-directory "${PROJECT_ROOT}/docker" "$@"
}

ensure_env() {
  step ".env 확인"
  if [[ -f "${PROJECT_ROOT}/.env" ]]; then
    log_success ".env 파일이 이미 존재합니다"
    return
  fi

  if [[ ! -f "${PROJECT_ROOT}/.env.example" ]]; then
    log_warn ".env.example 이 없어 기본 DATABASE_URL 로 .env 를 생성합니다"
    cat > "${PROJECT_ROOT}/.env" <<EOF
DATABASE_URL=${DATABASE_URL_DEFAULT}
HOST=127.0.0.1
PORT=8000
RUST_LOG=info
EOF
  else
    cp "${PROJECT_ROOT}/.env.example" "${PROJECT_ROOT}/.env"
    log_success ".env.example 을 복사해 .env 를 생성했습니다"
  fi
}

ensure_initdb_dir() {
  # compose 의 volume 마운트 대상 (없으면 docker 가 파일로 만들 수 있어 미리 디렉터리 생성)
  mkdir -p "${PROJECT_ROOT}/docker/initdb"
}

start_mysql() {
  step "Docker MySQL 기동 (${MYSQL_SERVICE})"
  require_command docker

  if ! docker info >/dev/null 2>&1; then
    log_error "Docker 데몬에 연결할 수 없습니다. Docker 가 실행 중인지 확인하세요."
    exit 1
  fi

  ensure_initdb_dir
  compose up -d "${MYSQL_SERVICE}"
  wait_for_mysql
}

wait_for_mysql() {
  step "MySQL healthy 대기 (timeout: ${MYSQL_WAIT_TIMEOUT}s)"
  local elapsed=0
  local interval=2
  local status=""

  while (( elapsed < MYSQL_WAIT_TIMEOUT )); do
    # 컨테이너가 아직 생성되지 않았을 수 있음
    if docker inspect -f '{{.State.Health.Status}}' "${MYSQL_CONTAINER}" >/dev/null 2>&1; then
      status="$(docker inspect -f '{{.State.Health.Status}}' "${MYSQL_CONTAINER}" 2>/dev/null || true)"
      if [[ "${status}" == "healthy" ]]; then
        log_success "MySQL 이 준비되었습니다 (container: ${MYSQL_CONTAINER})"
        return
      fi
      log_info "상태: ${status:-unknown} (${elapsed}s)"
    else
      # healthcheck 가 없는 이미지/구버전 대비: mysqladmin ping 시도
      if docker exec "${MYSQL_CONTAINER}" mysqladmin ping -h localhost -u test -ptest --silent >/dev/null 2>&1; then
        log_success "MySQL 이 응답합니다 (container: ${MYSQL_CONTAINER})"
        return
      fi
      log_info "컨테이너 준비 중... (${elapsed}s)"
    fi
    sleep "${interval}"
    elapsed=$((elapsed + interval))
  done

  log_error "MySQL 이 ${MYSQL_WAIT_TIMEOUT}초 내에 준비되지 않았습니다"
  compose ps || true
  compose logs --tail=50 "${MYSQL_SERVICE}" || true
  exit 1
}

stop_mysql() {
  step "MySQL 중지"
  require_command docker
  compose stop "${MYSQL_SERVICE}" || true
  log_success "MySQL 중지 완료"
}

down_mysql() {
  local remove_volumes="${1:-false}"
  step "MySQL 정리 (volumes=${remove_volumes})"
  require_command docker
  if [[ "${remove_volumes}" == "true" ]]; then
    compose down -v --remove-orphans
  else
    compose down --remove-orphans
  fi
  log_success "정리 완료"
}

format_code() {
  step "코드 포맷팅 (cargo fmt)"
  require_command cargo
  cargo fmt --all
  log_success "포맷팅 완료"
}

build_code() {
  step "빌드 (cargo build --profile dev)"
  require_command cargo
  # dotenv 사용 시 테스트/실행 일관성
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
  ensure_env
  start_mysql
  format_code
  build_code
  run_tests
  printf '\n'
  log_success "전체 파이프라인 완료 (MySQL + format + build + test)"
  log_info "MySQL 은 계속 실행 중입니다. 중지: ./scripts/run.sh down"
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
      mysql|db)
        start_mysql
        ;;
      down)
        down_mysql false
        ;;
      down-v)
        down_mysql true
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
