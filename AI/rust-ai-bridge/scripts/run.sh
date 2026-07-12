#!/usr/bin/env bash
#
# rust-ai-bridge 빌드/테스트/실행 스크립트
#
# 사용법:
#   scripts/run.sh [fmt|check|build|test|ci|serve|serve-compose|infra-down|gateway|inspect|agent-token|audit|eval|all] [-- 추가인자]
#
#   fmt          cargo fmt (코드 포맷팅)
#   check        cargo clippy --workspace --all-targets (무경고여야 함)
#   build        릴리스 바이너리 빌드 (target/release/{gateway,auditctl,agentctl,evalctl})
#   test         cargo test --workspace
#   ci           품질 게이트 (fmt --check · clippy · test)
#   serve        게이트웨이를 HTTP+콘솔+메트릭으로 기동 (Ctrl-C 로 종료). 로컬 개발용 --allow-mock-backends
#   serve-compose  PostgreSQL + Redis + 게이트웨이를 한 번에 기동
#   infra-down     Compose 인프라를 종료 (데이터 볼륨은 유지)
#   gateway      MCP 게이트웨이 서버 실행 (stdio). 예: scripts/run.sh gateway -- --http :8080
#   inspect      MCP Inspector(웹 UI)를 띄워 실행 중인 게이트웨이(:8080)의 도구를 나열/호출.
#                serve 가 떠 있어야 하며 Node.js(npx)가 필요합니다. INSPECT_USER 로 접속 주체 지정
#   agent-token  에이전트 토큰 발급/해시. 예: scripts/run.sh agent-token -- --user agent-support-bot
#                발급한 평문 토큰은 에이전트에만 전달하고 token_sha256 만 principal.yaml 에 넣으세요
#   audit        감사 로그 조회. 예: scripts/run.sh audit -- log --denied
#   eval         품질 평가 턴/피드백/골든셋. 예: scripts/run.sh eval -- run --suite erp-read
#   all          워크스페이스 전체를 빌드 → 테스트 → 서버 기동 (기본값)
#                fmt → (clippy) → cargo build --release → cargo test → 게이트웨이 서버 실행
#                추가 인자는 게이트웨이에 전달됩니다. Ctrl-C 로 종료
#
# all 환경변수:
#   CLIPPY=1     빌드 전에 clippy(무경고) 게이트를 함께 돌립니다 (기본 0 = 건너뜀)
#   SKIP_TESTS=1 테스트를 건너뛰고 빌드 후 바로 서버를 띄웁니다 (기본 0)
#   COMPOSE=1    PostgreSQL + Redis 를 Compose 로 함께 띄워 붙습니다 (기본 0 = SQLite 단독)
#
# serve / serve-compose / all 환경변수:
#   HTTP_ADDR       MCP HTTP 전송 주소 (기본 :8080)
#   CONSOLE_ADDR    운영 콘솔 주소 (기본 :8081)
#   METRICS_ADDR    Prometheus 메트릭 주소 (기본 :9464)
#   CONSOLE_USER    콘솔 접속 주체. 개발 전용 (기본 manager-01)
#   INSPECT_USER    MCP Inspector 접속 주체 (기본 emp-sales-01)
#   BUSINESS_HOURS  1 이면 업무시간(9~18시) 정책을 그대로 적용 (기본 0 = 시간 제한 해제)
#                   참고: business-hours-only 규칙은 **주말도 업무시간이 아님**으로 봅니다.
#                   토·일요일에는 시간 제한을 꺼도(0) 이 규칙이 여전히 호출을 거부합니다
#                   (Go 판과 동일한 동작). 주말 시연이 필요하면 정책에서 그 규칙을 빼십시오.
#   DB_PATH         감사 SQLite 경로 (기본 audit.db; --postgres-dsn 사용 시 무시)
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# Compose 파일은 docker/ 아래에 있습니다. docker compose 는 하위 디렉터리를 뒤지지 않으므로
# 리포지토리 루트에서 부를 때는 -f 로 명시해야 합니다.
COMPOSE_FILE="$APP_DIR/docker/compose.yaml"

DB_PATH="${DB_PATH:-audit.db}"

HTTP_ADDR="${HTTP_ADDR:-:8080}"
CONSOLE_ADDR="${CONSOLE_ADDR:-:8081}"
METRICS_ADDR="${METRICS_ADDR:-:9464}"
CONSOLE_USER="${CONSOLE_USER:-manager-01}"
INSPECT_USER="${INSPECT_USER:-emp-sales-01}"
BUSINESS_HOURS="${BUSINESS_HOURS:-0}"
CLIPPY="${CLIPPY:-0}"
SKIP_TESTS="${SKIP_TESTS:-0}"
COMPOSE="${COMPOSE:-0}"
BUSINESS_PURPOSE="${BUSINESS_PURPOSE:-sales_followup}"
LLM_DESTINATION="${LLM_DESTINATION:-internal}"
# Compose 파일의 ${...} 보간이 이 값들을 그대로 보도록 export 합니다.
# (그래야 아래에서 만드는 DSN 과 컨테이너가 실제로 여는 포트가 어긋나지 않습니다.)
export POSTGRES_PORT="${POSTGRES_PORT:-5432}"
export POSTGRES_DB="${POSTGRES_DB:-gateway}"
export POSTGRES_USER="${POSTGRES_USER:-gateway}"
export POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-gateway-dev-password}"
export REDIS_PORT="${REDIS_PORT:-6379}"

info() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
err() { printf '\033[1;31m==>\033[0m %s\n' "$*" >&2; }

require_cargo() {
	if ! command -v cargo >/dev/null 2>&1; then
		err "cargo 명령을 찾을 수 없습니다. Rust 툴체인(rustup)을 설치하세요."
		exit 1
	fi
}

# addr_url 은 ":9090" 같은 수신 주소를 접속 가능한 URL로 바꿉니다.
# 호스트가 비어 있으면 모든 인터페이스에서 듣는다는 뜻이므로 localhost로 붙습니다.
addr_url() {
	local addr="$1"
	[[ "$addr" == :* ]] && addr="localhost$addr"
	printf 'http://%s' "$addr"
}

# build → test → serve 를 이어 돌릴 때 fmt 가 세 번 도는 것을 막습니다.
FMT_DONE=0

cmd_fmt() {
	[[ "$FMT_DONE" == "1" ]] && return 0
	FMT_DONE=1
	info "fmt"
	cd "$APP_DIR"
	# rustfmt 컴포넌트가 없으면 경고만 하고 넘어갑니다.
	if cargo fmt --version >/dev/null 2>&1; then
		cargo fmt
	else
		err "rustfmt 가 없어 건너뜁니다 (rustup component add rustfmt)"
	fi
}

cmd_check() {
	info "clippy (무경고여야 함)"
	cd "$APP_DIR"
	cargo clippy --workspace --all-targets -- -D warnings
}

cmd_build() {
	cmd_fmt
	info "build (release)"
	cd "$APP_DIR"
	cargo build --workspace --release
	info "바이너리: target/release/{gateway,auditctl,agentctl,evalctl}"
}

cmd_test() {
	cmd_fmt
	info "test"
	cd "$APP_DIR"
	cargo test --workspace
}

# cmd_ci 는 로컬·CI 공용 품질 게이트입니다.
#   fmt 검사(수정하지 않고 diff 확인) → clippy(무경고) → test
cmd_ci() {
	cd "$APP_DIR"
	info "ci: fmt --check"
	if cargo fmt --version >/dev/null 2>&1; then
		cargo fmt --check
	else
		err "rustfmt 가 없어 fmt 검사를 건너뜁니다"
	fi
	cmd_check
	info "ci: test"
	cargo test --workspace
	info "ci: 통과"
}

cmd_gateway() {
	info "gateway (stdio, db=$DB_PATH)"
	cd "$APP_DIR"
	cargo run --quiet --bin gateway -- --db "$DB_PATH" "$@"
}

GATEWAY_PID=""

# cleanup 은 serve 가 띄운 배경 프로세스를 정리합니다.
cleanup() {
	local pid
	for pid in "$GATEWAY_PID"; do
		if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
			kill "$pid" 2>/dev/null || true
			wait "$pid" 2>/dev/null || true
		fi
	done
}

# _start_gateway 는 게이트웨이를 배경으로 띄웁니다.
# systems.yaml 이 memory/빈 base_url 인 개발 구성이므로 --allow-mock-backends 를 켭니다.
# 운영에서는 실제 base_url 을 넣고 이 플래그를 빼십시오. 인자는 게이트웨이에 전달됩니다.
_start_gateway() {
	local hours=()
	if [[ "$BUSINESS_HOURS" != "1" ]]; then
		hours=(--business-start 0 --business-end 0)
		info "업무시간 정책 해제 (BUSINESS_HOURS=1 로 켜세요)"
		# 주말에는 시간 제한을 꺼도 business-hours-only 규칙이 여전히 막습니다.
		local dow
		dow="$(date +%u)" # 6=토, 7=일
		if [[ "$dow" -ge 6 ]]; then
			err "주말입니다: business-hours-only 규칙이 여전히 호출을 거부합니다(Go 판과 동일)."
			err "     주말 시연이 필요하면 config/policies.yaml 에서 그 규칙을 빼십시오."
		fi
	fi

	info "gateway (http=$HTTP_ADDR, console=$CONSOLE_ADDR, db=$DB_PATH)"
	info "운영 콘솔: $(addr_url "$CONSOLE_ADDR")  (X-User-Id: $CONSOLE_USER 로 고정, 개발 전용)"
	info "MCP 엔드포인트: $(addr_url "$HTTP_ADDR")  (X-User-Id 헤더 필요)"
	info "메트릭: $(addr_url "$METRICS_ADDR")/metrics"

	./target/release/gateway \
		--db "$DB_PATH" \
		--http "$HTTP_ADDR" \
		--console "$CONSOLE_ADDR" \
		--console-user "$CONSOLE_USER" \
		--metrics "$METRICS_ADDR" \
		--allow-mock-backends \
		--business-purpose "$BUSINESS_PURPOSE" \
		--llm-destination "$LLM_DESTINATION" \
		${hours[@]+"${hours[@]}"} \
		"$@" &
	GATEWAY_PID=$!
}

# _wait_ready 는 콘솔의 /readyz 가 200 을 줄 때까지 최대 30초 기다립니다.
# 게이트웨이가 그 사이 죽으면 즉시 실패로 처리합니다.
_wait_ready() {
	local url i
	url="$(addr_url "$CONSOLE_ADDR")/readyz"
	for ((i = 0; i < 60; i++)); do
		if [[ -n "$GATEWAY_PID" ]] && ! kill -0 "$GATEWAY_PID" 2>/dev/null; then
			err "gateway 가 기동 중 종료되었습니다"
			return 1
		fi
		if curl -fsS -o /dev/null --max-time 2 "$url" 2>/dev/null; then
			info "서버 준비 완료 (readyz OK). Ctrl-C 로 종료합니다."
			return 0
		fi
		sleep 0.5
	done
	err "경고: 30초 안에 $url 이 준비되지 않았습니다 (서버는 계속 실행합니다)"
	return 0
}

# _serve_gateway_wait 는 게이트웨이를 띄우고 종료를 기다립니다.
# 추가 인자는 게이트웨이에 전달됩니다. 호출자가 먼저 cmd_build 를 해야 합니다.
_serve_gateway_wait() {
	GATEWAY_PID=""
	trap cleanup EXIT INT TERM

	cd "$APP_DIR"
	_start_gateway "$@"
	_wait_ready || return 1

	wait "$GATEWAY_PID" || {
		local code=$?
		GATEWAY_PID=""
		err "gateway 종료 (코드 $code)"
		return "$code"
	}
	GATEWAY_PID=""
}

# cmd_serve 는 게이트웨이를 HTTP+콘솔로 띄웁니다.
# 추가 인자는 게이트웨이에 전달됩니다 (Ctrl-C 로 종료).
cmd_serve() {
	cmd_build
	_serve_gateway_wait "$@"
}

require_docker_compose() {
	if ! command -v docker >/dev/null 2>&1 || ! docker compose version >/dev/null 2>&1; then
		err "Docker Compose v2(docker compose)를 찾을 수 없습니다. Docker Desktop 또는 Compose plugin을 설치하세요."
		exit 1
	fi
}

compose() { docker compose -f "$COMPOSE_FILE" "$@"; }

# port_busy 는 호스트 포트가 이미 물려 있는지 봅니다.
# 충돌한 채로 Compose 를 띄우면 컨테이너가 네트워크 없이 올라와
# "Name or service not known" 같은 엉뚱한 DNS 오류로 실패합니다.
port_busy() { timeout 1 bash -c "cat </dev/null >/dev/tcp/127.0.0.1/$1" 2>/dev/null; }

# _start_infra 는 Compose 인프라를 올리고 PG_DSN/REDIS_URL 전역을 채웁니다.
PG_DSN=""
REDIS_URL=""
_start_infra() {
	require_docker_compose
	cd "$APP_DIR"

	# 이미 우리 인프라가 떠 있으면 그 포트는 우리 것이므로 검사하지 않습니다.
	if [[ -z "$(compose ps -q postgres 2>/dev/null)" ]]; then
		local busy=0
		if port_busy "$POSTGRES_PORT"; then
			err "포트 $POSTGRES_PORT (PostgreSQL) 이 이미 사용 중입니다. POSTGRES_PORT 로 다른 포트를 지정하세요."
			busy=1
		fi
		if port_busy "$REDIS_PORT"; then
			err "포트 $REDIS_PORT (Redis) 이 이미 사용 중입니다. REDIS_PORT 로 다른 포트를 지정하세요."
			busy=1
		fi
		if [[ "$busy" == "1" ]]; then
			err "예) POSTGRES_PORT=15432 REDIS_PORT=16379 scripts/run.sh serve-compose"
			exit 1
		fi
	fi

	info "PostgreSQL + Redis 기동 및 healthcheck 대기"
	compose up -d --wait postgres redis
	PG_DSN="postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@localhost:${POSTGRES_PORT}/${POSTGRES_DB}?sslmode=disable"
	REDIS_URL="redis://localhost:${REDIS_PORT}/0"
	info "분산 저장소 사용 (PostgreSQL localhost:$POSTGRES_PORT, Redis localhost:$REDIS_PORT)"
}

# Compose는 상태 저장 인프라를 관리하고, 게이트웨이가 애플리케이션 프로세스를 담당합니다.
# Ctrl-C 시 앱은 종료하지만 DB 데이터와 컨테이너는 재사용하도록 남깁니다.
cmd_serve_compose() {
	cmd_build
	_start_infra
	AUDITCTL_POSTGRES_DSN="$PG_DSN" _serve_gateway_wait \
		--postgres-dsn "$PG_DSN" --redis-url "$REDIS_URL" "$@"
}

cmd_infra_down() {
	require_docker_compose
	cd "$APP_DIR"
	compose down
}

# cmd_inspect 는 MCP Inspector(웹 UI)를 띄웁니다.
#
# 게이트웨이의 8080 은 REST 가 아니라 MCP Streamable HTTP(JSON-RPC) 라서 브라우저로
# 그냥 열면 세션 핸드셰이크가 필요합니다. Inspector 가 핸드셰이크를 대신 해주고
# tools/list·tools/call 을 폼 UI 로 노출합니다("MCP 용 Swagger").
# 게이트웨이는 X-User-Id 헤더로 주체를 구분하므로 UI 에서 헤더를 채워야 합니다.
cmd_inspect() {
	if ! command -v npx >/dev/null 2>&1; then
		err "npx(Node.js)를 찾을 수 없습니다. Node 18+ 를 설치하세요."
		exit 1
	fi
	local url cfg
	url="$(addr_url "$HTTP_ADDR")"
	cd "$APP_DIR"
	if ! curl -s -o /dev/null "$url" 2>/dev/null; then
		err "경고: $url 에 게이트웨이가 응답하지 않습니다. 먼저 'scripts/run.sh serve' 로 띄우세요."
	fi
	mkdir -p dist
	cfg="dist/inspector-config.json"
	cat >"$cfg" <<JSON
{
  "mcpServers": {
    "rust-ai-bridge": {
      "type": "streamable-http",
      "url": "$url",
      "headers": { "X-User-Id": "$INSPECT_USER" }
    }
  }
}
JSON
	info "MCP Inspector 시작 (브라우저가 자동으로 열립니다)"
	info "미리 채워진 연결: Streamable HTTP  $url  (X-User-Id: $INSPECT_USER)"
	info "다른 주체로 보려면 INSPECT_USER 를 바꿔 다시 실행하세요 (예: INSPECT_USER=manager-01)"
	npx @modelcontextprotocol/inspector --config "$cfg" --server rust-ai-bridge "$@"
}

# cmd_agent_token 은 에이전트 토큰을 발급하거나 해시합니다.
# 예) scripts/run.sh agent-token -- --user agent-support-bot   (새 토큰 발급)
#     scripts/run.sh agent-token -- hash <token>               (기존 토큰 해시)
# 인자가 없거나 서브커맨드를 명시하지 않으면 token 을 기본으로 씁니다.
cmd_agent_token() {
	cd "$APP_DIR"
	case "${1:-}" in
	token | hash | help | -h | --help) cargo run --quiet --bin agentctl -- "$@" ;;
	*) cargo run --quiet --bin agentctl -- token "$@" ;;
	esac
}

cmd_audit() {
	info "audit (db=$DB_PATH)"
	cd "$APP_DIR"
	# --db 는 전역 인자라 서브커맨드 앞뒤 어디에 와도 됩니다.
	cargo run --quiet --bin auditctl -- --db "$DB_PATH" "$@"
}

cmd_eval() {
	local eval_db="${EVAL_DB:-eval.db}"
	info "eval (db=$eval_db)"
	cd "$APP_DIR"
	cargo run --quiet --bin evalctl -- --db "$eval_db" "$@"
}

# cmd_all 은 워크스페이스 전체를 빌드 → 테스트 → 서버 기동 순으로 한 번에 돌립니다.
# 테스트가 실패하면 서버를 띄우지 않고 그 자리에서 멈춥니다 (set -e).
# 추가 인자는 게이트웨이에 전달됩니다.
cmd_all() {
	cmd_fmt
	if [[ "$CLIPPY" == "1" ]]; then
		cmd_check
	fi

	cmd_build

	if [[ "$SKIP_TESTS" == "1" ]]; then
		info "test 건너뜀 (SKIP_TESTS=1)"
	else
		info "test"
		cd "$APP_DIR"
		cargo test --workspace
		info "test 통과"
	fi

	info "서버 기동"
	if [[ "$COMPOSE" == "1" ]]; then
		_start_infra
		AUDITCTL_POSTGRES_DSN="$PG_DSN" _serve_gateway_wait \
			--postgres-dsn "$PG_DSN" --redis-url "$REDIS_URL" "$@"
	else
		_serve_gateway_wait "$@"
	fi
}

main() {
	require_cargo
	local sub="${1:-all}"
	shift || true
	# "-- 추가인자" 형태에서 -- 를 건너뜁니다.
	if [[ "${1:-}" == "--" ]]; then shift; fi

	case "$sub" in
	fmt) cmd_fmt ;;
	check | clippy) cmd_check ;;
	build) cmd_build ;;
	test) cmd_test ;;
	ci) cmd_ci ;;
	serve) cmd_serve "$@" ;;
	serve-compose) cmd_serve_compose "$@" ;;
	infra-down) cmd_infra_down ;;
	gateway) cmd_gateway "$@" ;;
	inspect) cmd_inspect "$@" ;;
	agent-token) cmd_agent_token "$@" ;;
	audit) cmd_audit "$@" ;;
	eval) cmd_eval "$@" ;;
	all) cmd_all "$@" ;;
	*)
		grep '^#' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
		exit 1
		;;
	esac
}

main "$@"
