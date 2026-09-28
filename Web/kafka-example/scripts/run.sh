#!/usr/bin/env bash
# Compatible with macOS Bash 3.2 and Linux Bash.
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

log() { printf '\n[run] %s\n' "$*"; }
die() { printf '\n[run] 오류: %s\n' "$*" >&2; exit 1; }
usage() {
    cat <<'EOF'
사용법: ./scripts/run.sh [--check] [--external-kafka]

기본: 의존성 설치 → Rust/프론트엔드 포맷팅 → 빌드 → 테스트 → Kafka → 백엔드 → Vite
  --check           포맷팅·빌드·검증만 실행 (Docker 불필요)
  --external-kafka  Compose 대신 KAFKA_BROKERS의 기존 Kafka 사용 (rust 토픽 필요)
  -h, --help        도움말

환경변수:
  WEB_ADDR          백엔드 주소 (기본: 127.0.0.1:3000, Vite 프록시도 연동)
  FRONTEND_PORT     프론트엔드 포트 (기본: 5173)
  KAFKA_BROKERS     브로커 주소 (기본: 127.0.0.1:9092)

Ctrl+C로 종료합니다. 이 실행이 시작한 Kafka만 중지하며 볼륨은 보존합니다.
백엔드/프론트엔드 로그: .run/backend.log, .run/frontend.log
EOF
}

CHECK_ONLY=0
EXTERNAL_KAFKA=0
for arg in "$@"; do
    case "$arg" in
        --check) CHECK_ONLY=1 ;;
        --external-kafka) EXTERNAL_KAFKA=1 ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; die "알 수 없는 옵션: $arg" ;;
    esac
done

export WEB_ADDR="${WEB_ADDR:-127.0.0.1:3000}"
# Compose publishes IPv4 only; avoid an initial ::1 connection error on macOS.
export KAFKA_BROKERS="${KAFKA_BROKERS:-127.0.0.1:9092}"
export FRONTEND_DIST="${FRONTEND_DIST:-$ROOT_DIR/frontend/dist}"
export FRONTEND_PORT="${FRONTEND_PORT:-5173}"
COMPOSE=(docker compose --project-directory "$ROOT_DIR" -f "$ROOT_DIR/compose.yaml")
BACKEND_PID=""
FRONTEND_PID=""
STOP_KAFKA=0

cleanup() {
    local status=$? pid attempt
    trap - EXIT INT TERM ERR
    set +e
    for pid in "$FRONTEND_PID" "$BACKEND_PID"; do
        [ -n "$pid" ] && kill -TERM "$pid" 2>/dev/null
    done
    # Bound shutdown time even if a child stops responding.
    for attempt in 1 2 3 4 5; do
        local alive=0
        for pid in "$FRONTEND_PID" "$BACKEND_PID"; do
            if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then alive=1; fi
        done
        [ "$alive" -eq 0 ] && break
        sleep 1
    done
    for pid in "$FRONTEND_PID" "$BACKEND_PID"; do
        if [ -n "$pid" ]; then
            kill -KILL "$pid" 2>/dev/null
            wait "$pid" 2>/dev/null
        fi
    done
    if [ "$STOP_KAFKA" -eq 1 ]; then
        log "이 실행에서 시작한 Kafka 중지 (메시지 볼륨 보존)"
        "${COMPOSE[@]}" stop -t 10 kafka
    fi
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'printf "\n[run] 실행 실패: 줄 %s (종료 코드 %s)\n" "$LINENO" "$?" >&2' ERR

log "필수 도구 확인"
for command in cargo node npm cmake pkg-config; do
    command -v "$command" >/dev/null 2>&1 || die "$command 명령이 없습니다. README의 사전 준비를 확인하세요."
done
node -e 'const [major, minor] = process.versions.node.split(".").map(Number); process.exit(major > 22 || (major === 22 && minor >= 12) ? 0 : 1)' \
    || die "Node.js 22.12 이상이 필요합니다."
cargo fmt --version >/dev/null

if [ "$CHECK_ONLY" -eq 0 ]; then
    command -v curl >/dev/null 2>&1 || die "curl이 필요합니다."
    # Binding first catches occupied ports without terminating existing servers.
    node --input-type=module <<'JS'
import net from "node:net";
const backend = new URL(`http://${process.env.WEB_ADDR}`);
const frontendPort = Number(process.env.FRONTEND_PORT);
if (!Number.isInteger(frontendPort) || frontendPort < 1 || frontendPort > 65535) {
  throw new Error("FRONTEND_PORT는 1~65535 정수여야 합니다.");
}
const servers = [];
try {
  for (const [host, port] of [
    [backend.hostname.replace(/^\[|\]$/g, ""), Number(backend.port || 80)],
    ["127.0.0.1", frontendPort],
  ]) {
    const server = net.createServer();
    await new Promise((resolve, reject) => {
      server.once("error", reject);
      server.listen(port, host, resolve);
    });
    servers.push(server);
  }
} catch (error) {
  console.error("[run] 포트를 사용할 수 없습니다. WEB_ADDR 또는 FRONTEND_PORT를 바꾸세요.", error.message);
  process.exitCode = 1;
} finally {
  for (const server of servers) server.close();
}
JS
    if [ "$EXTERNAL_KAFKA" -eq 0 ]; then
        case "$KAFKA_BROKERS" in
            localhost:9092|127.0.0.1:9092) ;;
            *) die "다른 브로커 주소는 --external-kafka 옵션으로 실행하세요." ;;
        esac
        command -v docker >/dev/null 2>&1 || die "Docker가 필요합니다."
        "${COMPOSE[@]}" version >/dev/null
        docker info >/dev/null 2>&1 || die "Docker 데몬을 먼저 시작하세요."
        "${COMPOSE[@]}" config --quiet
    fi
fi

log "프론트엔드 의존성 설치"
npm ci --prefix frontend
log "Rust 및 프론트엔드 코드 포맷팅"
cargo fmt --all
npm run format --prefix frontend
log "Rust 전체 타깃 및 프론트엔드 빌드 (TypeScript 타입 검사 포함)"
cargo build --locked --all-targets
npm run build --prefix frontend
log "Rust 테스트 실행 (librdkafka MockCluster 사용)"
cargo test --locked --all-targets
log "프론트엔드 검증 완료: 타입 검사 및 프로덕션 빌드 (별도 테스트 스위트 없음)"
if [ "$CHECK_ONLY" -eq 1 ]; then
    log "포맷팅·빌드·검증 완료"
    exit 0
fi

if [ "$EXTERNAL_KAFKA" -eq 0 ]; then
    log "Kafka 이미지 준비 및 시작 (준비 상태 대기: 최대 180초)"
    if [ -z "$("${COMPOSE[@]}" ps --status running -q kafka)" ]; then STOP_KAFKA=1; fi
    "${COMPOSE[@]}" up -d --wait --wait-timeout 180 kafka
    log "rust 토픽 초기화"
    "${COMPOSE[@]}" run --rm --no-deps kafka-init
else
    log "기존 Kafka 사용: $KAFKA_BROKERS"
fi

mkdir -p "$ROOT_DIR/.run"
TARGET_DIR="$(cargo metadata --no-deps --format-version 1 | node -e '
let input = ""; process.stdin.on("data", chunk => input += chunk);
process.stdin.on("end", () => console.log(JSON.parse(input).target_directory));
')"
BACKEND_URL="$(node -e 'console.log("http://" + process.env.WEB_ADDR.replace(/^(0\.0\.0\.0|\[::\]):/, "127.0.0.1:"))')"
FRONTEND_URL="http://127.0.0.1:$FRONTEND_PORT"

wait_for_http() {
    local url=$1 pid=$2 logfile=$3 deadline=$((SECONDS + 60))
    while [ "$SECONDS" -lt "$deadline" ]; do
        if ! kill -0 "$pid" 2>/dev/null; then
            tail -n 40 "$logfile" >&2
            die "서버가 준비 중 종료되었습니다: $url"
        fi
        if curl --noproxy '*' --fail --silent --output /dev/null --max-time 10 "$url"; then return 0; fi
        sleep 1
    done
    tail -n 40 "$logfile" >&2
    die "서버 준비 시간 초과: $url"
}

log "백엔드 시작"
"$TARGET_DIR/debug/web" >"$ROOT_DIR/.run/backend.log" 2>&1 &
BACKEND_PID=$!
wait_for_http "$BACKEND_URL/api/health" "$BACKEND_PID" "$ROOT_DIR/.run/backend.log"

log "프론트엔드 시작"
# Run Vite directly so the tracked PID is the server, not an npm wrapper.
(
    cd "$ROOT_DIR/frontend"
    exec node node_modules/vite/bin/vite.js --host 127.0.0.1 --port "$FRONTEND_PORT" --strictPort
) >"$ROOT_DIR/.run/frontend.log" 2>&1 &
FRONTEND_PID=$!
wait_for_http "$FRONTEND_URL" "$FRONTEND_PID" "$ROOT_DIR/.run/frontend.log"
wait_for_http "$FRONTEND_URL/api/health" "$FRONTEND_PID" "$ROOT_DIR/.run/frontend.log"

log "실행 완료 — 프론트엔드: $FRONTEND_URL / 백엔드: $BACKEND_URL"
log "로그: $ROOT_DIR/.run/{backend,frontend}.log — 종료: Ctrl+C"
while kill -0 "$BACKEND_PID" 2>/dev/null && kill -0 "$FRONTEND_PID" 2>/dev/null; do sleep 1; done
die "백엔드 또는 프론트엔드가 종료되었습니다. .run 로그를 확인하세요."
