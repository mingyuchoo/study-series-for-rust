#!/usr/bin/env bash
#
# rust-ai-bridge Docker 이미지 빌드/컨테이너 기동 스크립트
#
# 사용법:
#   scripts/docker.sh [build|up|down|run|stop|restart|logs|ps|exec|shell|clean] [-- 추가인자]
#
#   build    docker/Dockerfile 로 이미지를 빌드 (컨텍스트는 리포지토리 루트)
#            추가인자는 docker build 로 전달됩니다. 예: scripts/docker.sh build -- --no-cache
#   up       PostgreSQL + Redis + 게이트웨이를 Compose 로 기동 (healthcheck 통과까지 대기)
#            운영에 가까운 구성입니다. 감사·승인·워크플로 상태가 PostgreSQL/Redis 에 남습니다
#   down     Compose 스택 종료. 데이터 볼륨은 유지됩니다 (지우려면 clean)
#   run      게이트웨이 컨테이너 하나만 기동 (SQLite 단독, 인프라 없음)
#            이미지가 없으면 먼저 빌드합니다
#   stop     run 으로 띄운 단독 컨테이너를 종료·삭제
#   restart  Compose 스택을 다시 빌드해 재기동 (up 과 동일하지만 이미지를 새로 만듭니다)
#   logs     게이트웨이 로그 추적 (Ctrl-C 로 빠져나옴). 예: scripts/docker.sh logs -- --tail 100
#   ps       컨테이너 상태 확인
#   exec     컨테이너 안에서 CLI 실행. 예: scripts/docker.sh exec -- auditctl log --denied
#            auditctl 은 실행 중인 모드(Compose=PostgreSQL / 단독=SQLite)에 맞게 자동 연결됩니다
#   shell    컨테이너 셸(/bin/bash) 진입
#   clean    Compose 스택 + 데이터 볼륨 + 이미지를 모두 삭제 (되돌릴 수 없습니다)
#
# 환경변수:
#   IMAGE            이미지 이름 (기본 rust-ai-bridge)
#   TAG              이미지 태그 (기본 latest)
#   HTTP_PORT        MCP Streamable HTTP 호스트 포트 (기본 8080)
#   CONSOLE_PORT     운영 콘솔 호스트 포트 (기본 8081)
#   METRICS_PORT     Prometheus 메트릭 호스트 포트 (기본 9464)
#   CONSOLE_USER     콘솔 접속 주체. 개발 전용 (기본 manager-01)
#   BUSINESS_HOURS   1 이면 업무시간(9~18시) 정책을 그대로 적용 (기본 0 = 시간 제한 해제)
#                    참고: business-hours-only 규칙은 **주말도 업무시간이 아님**으로 봅니다.
#                    토·일요일에는 시간 제한을 꺼도(0) 이 규칙이 여전히 호출을 거부합니다
#   BUSINESS_PURPOSE 정책 컨텍스트의 업무 목적 (기본 sales_followup)
#   LLM_DESTINATION  정책 컨텍스트의 LLM 목적지 (기본 internal)
#   RUST_LOG         로그 레벨 (기본 info)
#   POSTGRES_PORT / POSTGRES_DB / POSTGRES_USER / POSTGRES_PASSWORD / REDIS_PORT
#                    Compose 인프라 설정 (docker/compose.yaml 기본값과 동일)
#
# 호스트 포트가 겹치면 기동 전에 막습니다. 다른 스택(go-ai-bridge 등)이 5432/6379 를
# 이미 쓰고 있다면 포트를 바꿔서 띄우십시오:
#   POSTGRES_PORT=15432 REDIS_PORT=16379 scripts/docker.sh up
#
# 리포지토리 루트의 .env 가 있으면 컨테이너 환경으로 주입됩니다 (AZURE_OPENAI_* 등).
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
COMPOSE_FILE="$APP_DIR/docker/compose.yaml"
DOCKERFILE="$APP_DIR/docker/Dockerfile"

# compose.yaml 의 ${...} 보간이 보도록 전부 export 합니다.
export IMAGE="${IMAGE:-rust-ai-bridge}"
export TAG="${TAG:-latest}"
export HTTP_PORT="${HTTP_PORT:-8080}"
export CONSOLE_PORT="${CONSOLE_PORT:-8081}"
export METRICS_PORT="${METRICS_PORT:-9464}"
export CONSOLE_USER="${CONSOLE_USER:-manager-01}"
export BUSINESS_PURPOSE="${BUSINESS_PURPOSE:-sales_followup}"
export LLM_DESTINATION="${LLM_DESTINATION:-internal}"
export RUST_LOG="${RUST_LOG:-info}"
export POSTGRES_PORT="${POSTGRES_PORT:-5432}"
export POSTGRES_DB="${POSTGRES_DB:-gateway}"
export POSTGRES_USER="${POSTGRES_USER:-gateway}"
export POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-gateway-dev-password}"
export REDIS_PORT="${REDIS_PORT:-6379}"

BUSINESS_HOURS="${BUSINESS_HOURS:-0}"
CONTAINER_NAME="${CONTAINER_NAME:-rust-ai-bridge}"
DATA_VOLUME="${DATA_VOLUME:-rust-ai-bridge-data}"

# 컨테이너 안의 경로·주소는 고정입니다. 호스트 포트만 위 변수로 바꿉니다.
IN_HTTP=":8080"
IN_CONSOLE=":8081"
IN_METRICS=":9464"
IN_DB="/app/data/audit.db"

info() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
err() { printf '\033[1;31m==>\033[0m %s\n' "$*" >&2; }

require_docker() {
	if ! command -v docker >/dev/null 2>&1 || ! docker compose version >/dev/null 2>&1; then
		err "Docker Compose v2(docker compose)를 찾을 수 없습니다. Docker Desktop 또는 Compose plugin을 설치하세요."
		exit 1
	fi
	if ! docker info >/dev/null 2>&1; then
		err "Docker 데몬에 연결할 수 없습니다. Docker 를 먼저 실행하세요."
		exit 1
	fi
}

compose() { docker compose -f "$COMPOSE_FILE" "$@"; }

# 게이트웨이가 컨테이너 안에서 쓰는 DSN (호스트가 아니라 Compose 네트워크의 서비스 이름).
internal_pg_dsn() {
	printf 'postgres://%s:%s@postgres:5432/%s?sslmode=disable' \
		"$POSTGRES_USER" "$POSTGRES_PASSWORD" "$POSTGRES_DB"
}

# set_business_hours_flags 는 업무시간 정책을 끌 인자를 전역 HOURS 배열에 채웁니다.
# BUSINESS_HOURS=1 이면 비워 두어 게이트웨이 기본값(9~18시)이 그대로 적용됩니다.
HOURS=()
set_business_hours_flags() {
	HOURS=()
	[[ "$BUSINESS_HOURS" == "1" ]] && return 0

	HOURS=(--business-start 0 --business-end 0)
	info "업무시간 정책 해제 (BUSINESS_HOURS=1 로 켜세요)"
	local dow
	dow="$(date +%u)" # 6=토, 7=일
	if [[ "$dow" -ge 6 ]]; then
		err "주말입니다: business-hours-only 규칙이 여전히 호출을 거부합니다(Go 판과 동일)."
		err "     주말 시연이 필요하면 config/policies.yaml 에서 그 규칙을 빼십시오."
	fi
}

# port_busy 는 호스트 포트가 이미 물려 있는지 봅니다.
port_busy() { timeout 1 bash -c "cat </dev/null >/dev/tcp/127.0.0.1/$1" 2>/dev/null; }

# preflight_ports 는 기동 전에 호스트 포트 충돌을 잡습니다.
# 충돌한 채로 Compose 를 띄우면 컨테이너가 네트워크 없이 올라와
# "Name or service not known" 같은 엉뚱한 DNS 오류로 실패합니다.
# 다른 프로젝트(go-ai-bridge 등)가 5432/6379 를 쓰고 있으면 여기서 걸립니다.
preflight_ports() {
	local busy=0 spec port name var
	for spec in "$HTTP_PORT:MCP:HTTP_PORT" "$CONSOLE_PORT:콘솔:CONSOLE_PORT" "$METRICS_PORT:메트릭:METRICS_PORT" "$@"; do
		IFS=: read -r port name var <<<"$spec"
		if port_busy "$port"; then
			err "포트 $port ($name) 이 이미 사용 중입니다. $var 로 다른 포트를 지정하세요."
			busy=1
		fi
	done
	if [[ "$busy" == "1" ]]; then
		err "예) POSTGRES_PORT=15432 REDIS_PORT=16379 scripts/docker.sh up"
		exit 1
	fi
}

print_urls() {
	info "MCP 엔드포인트: http://localhost:$HTTP_PORT  (X-User-Id 헤더 필요)"
	info "운영 콘솔: http://localhost:$CONSOLE_PORT  (X-User-Id: $CONSOLE_USER 로 고정, 개발 전용)"
	info "메트릭: http://localhost:$METRICS_PORT/metrics"
}

image_exists() { docker image inspect "$IMAGE:$TAG" >/dev/null 2>&1; }

# compose_running / standalone_running 은 exec·shell 이 붙을 대상을 고릅니다.
compose_running() { [[ -n "$(compose ps -q gateway 2>/dev/null)" ]]; }
standalone_running() { [[ "$(docker inspect -f '{{.State.Running}}' "$CONTAINER_NAME" 2>/dev/null || true)" == "true" ]]; }

cmd_build() {
	info "이미지 빌드: $IMAGE:$TAG (컨텍스트 $APP_DIR)"
	docker build -f "$DOCKERFILE" -t "$IMAGE:$TAG" "$@" "$APP_DIR"
	info "완료: $IMAGE:$TAG"
}

# cmd_up 은 PostgreSQL + Redis + 게이트웨이를 한 번에 띄웁니다.
# --wait 로 세 서비스의 healthcheck 가 통과할 때까지 기다립니다.
cmd_up() {
	# 이미 우리 스택이 떠 있으면 그 포트는 우리 것이므로 검사하지 않습니다.
	compose_running || preflight_ports \
		"$POSTGRES_PORT:PostgreSQL:POSTGRES_PORT" "$REDIS_PORT:Redis:REDIS_PORT"
	info "Compose 기동 (PostgreSQL + Redis + 게이트웨이)"
	compose up -d --build --wait "$@"
	print_urls
	info "로그: scripts/docker.sh logs   ·   종료: scripts/docker.sh down"
}

cmd_down() {
	info "Compose 종료 (데이터 볼륨은 유지)"
	compose down "$@"
}

cmd_restart() {
	compose build gateway
	cmd_up
}

# cmd_run 은 인프라 없이 게이트웨이 컨테이너 하나만 띄웁니다 (감사 로그는 SQLite).
cmd_run() {
	image_exists || cmd_build

	if standalone_running; then
		err "이미 실행 중입니다: $CONTAINER_NAME (먼저 scripts/docker.sh stop)"
		exit 1
	fi
	docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
	preflight_ports

	set_business_hours_flags

	local env_file=()
	[[ -f "$APP_DIR/.env" ]] && env_file=(--env-file "$APP_DIR/.env")

	info "컨테이너 기동: $CONTAINER_NAME (SQLite $IN_DB, 볼륨 $DATA_VOLUME)"
	docker run -d \
		--name "$CONTAINER_NAME" \
		--init \
		-p "$HTTP_PORT:8080" \
		-p "$CONSOLE_PORT:8081" \
		-p "$METRICS_PORT:9464" \
		-v "$DATA_VOLUME:/app/data" \
		-e "RUST_LOG=$RUST_LOG" \
		${env_file[@]+"${env_file[@]}"} \
		"$IMAGE:$TAG" \
		gateway \
		--db "$IN_DB" \
		--http "$IN_HTTP" \
		--console "$IN_CONSOLE" \
		--console-user "$CONSOLE_USER" \
		--metrics "$IN_METRICS" \
		--allow-mock-backends \
		--business-purpose "$BUSINESS_PURPOSE" \
		--llm-destination "$LLM_DESTINATION" \
		${HOURS[@]+"${HOURS[@]}"} \
		"$@" >/dev/null

	wait_ready || return 1
	print_urls
	info "로그: scripts/docker.sh logs   ·   종료: scripts/docker.sh stop"
}

# wait_ready 는 콘솔의 /readyz 가 200 을 줄 때까지 최대 30초 기다립니다.
# 컨테이너가 그 사이 죽으면 즉시 실패로 처리하고 로그를 보여 줍니다.
wait_ready() {
	local url="http://localhost:$CONSOLE_PORT/readyz" i
	for ((i = 0; i < 60; i++)); do
		if ! standalone_running; then
			err "컨테이너가 기동 중 종료되었습니다. 마지막 로그:"
			docker logs --tail 30 "$CONTAINER_NAME" >&2 || true
			return 1
		fi
		if curl -fsS -o /dev/null --max-time 2 "$url" 2>/dev/null; then
			info "서버 준비 완료 (readyz OK)"
			return 0
		fi
		sleep 0.5
	done
	err "경고: 30초 안에 $url 이 준비되지 않았습니다 (컨테이너는 계속 실행합니다)"
	return 0
}

cmd_stop() {
	info "컨테이너 종료·삭제: $CONTAINER_NAME (볼륨 $DATA_VOLUME 은 유지)"
	docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || err "실행 중인 $CONTAINER_NAME 이 없습니다"
}

cmd_logs() {
	if compose_running; then
		compose logs -f "$@" gateway
	elif standalone_running; then
		docker logs -f "$@" "$CONTAINER_NAME"
	else
		err "실행 중인 게이트웨이가 없습니다 (up 또는 run 먼저)"
		exit 1
	fi
}

cmd_ps() {
	compose ps
	docker ps --filter "name=^/$CONTAINER_NAME$" --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'
}

# cmd_exec 은 컨테이너 안에서 auditctl·agentctl·evalctl 을 돌립니다.
# 저장소 위치가 모드마다 달라서(Compose=PostgreSQL, 단독=SQLite) 연결 인자를 대신 채워 줍니다.
cmd_exec() {
	if [[ $# -eq 0 ]]; then
		err "실행할 명령이 필요합니다. 예: scripts/docker.sh exec -- auditctl log --denied"
		exit 1
	fi
	if compose_running; then
		# compose exec 는 기본이 TTY 이므로, 파이프로 돌 때만 -T 로 끕니다.
		local no_tty=()
		[[ -t 0 ]] || no_tty=(-T)
		compose exec ${no_tty[@]+"${no_tty[@]}"} -e "AUDITCTL_POSTGRES_DSN=$(internal_pg_dsn)" gateway "$@"
	elif standalone_running; then
		local tty=()
		[[ -t 0 ]] && tty=(-it)
		local bin="$1"
		shift
		case "$bin" in
		auditctl) set -- --db "$IN_DB" "$@" ;;
		evalctl) set -- --db /app/data/eval.db "$@" ;;
		esac
		docker exec ${tty[@]+"${tty[@]}"} "$CONTAINER_NAME" "$bin" "$@"
	else
		err "실행 중인 게이트웨이가 없습니다 (up 또는 run 먼저)"
		exit 1
	fi
}

cmd_shell() {
	if compose_running; then
		compose exec gateway /bin/bash
	elif standalone_running; then
		docker exec -it "$CONTAINER_NAME" /bin/bash
	else
		err "실행 중인 게이트웨이가 없습니다 (up 또는 run 먼저)"
		exit 1
	fi
}

# cmd_clean 은 컨테이너·데이터 볼륨·이미지를 모두 지웁니다. 감사 로그도 함께 사라집니다.
cmd_clean() {
	info "Compose 스택과 볼륨 삭제"
	compose down -v --remove-orphans
	docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
	docker volume rm "$DATA_VOLUME" >/dev/null 2>&1 || true
	info "이미지 삭제: $IMAGE:$TAG"
	docker rmi "$IMAGE:$TAG" >/dev/null 2>&1 || true
	info "정리 완료"
}

main() {
	require_docker
	local sub="${1:-up}"
	shift || true
	# "-- 추가인자" 형태에서 -- 를 건너뜁니다.
	if [[ "${1:-}" == "--" ]]; then shift; fi

	case "$sub" in
	build) cmd_build "$@" ;;
	up) cmd_up "$@" ;;
	down) cmd_down "$@" ;;
	restart) cmd_restart ;;
	run) cmd_run "$@" ;;
	stop) cmd_stop ;;
	logs) cmd_logs "$@" ;;
	ps) cmd_ps ;;
	exec) cmd_exec "$@" ;;
	shell | sh) cmd_shell ;;
	clean) cmd_clean ;;
	*)
		grep '^#' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
		exit 1
		;;
	esac
}

main "$@"
