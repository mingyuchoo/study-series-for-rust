#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

BACKEND_PID=""
FRONTEND_PID=""

cleanup() {
  local pids="${BACKEND_PID:-} ${FRONTEND_PID:-}"

  echo ""
  echo "[run.sh] 프로세스를 종료합니다..."
  if [[ -n "${pids// /}" ]]; then
    kill $pids 2>/dev/null || true
    wait $pids 2>/dev/null || true
  fi
  echo "[run.sh] 모든 프로세스가 종료되었습니다."
}

trap cleanup EXIT INT TERM

# ── Backend 빌드 및 실행 ──
echo "[run.sh] Backend 빌드 중..."
(cd "$SCRIPT_DIR/backend" && cargo build --profile dev)

echo "[run.sh] Backend 실행 중... (http://localhost:4000)"
(cd "$SCRIPT_DIR/backend" && RUST_LOG=lib_db=info,lib_api=debug,actix_web=info cargo run -p bin-main) &
BACKEND_PID=$!

# ── Frontend 설치 및 실행 ──
echo "[run.sh] Frontend 의존성 설치 중..."
(cd "$SCRIPT_DIR/frontend" && bun install)

echo "[run.sh] Frontend 실행 중... (http://localhost:5173)"
(cd "$SCRIPT_DIR/frontend" && bun dev) &
FRONTEND_PID=$!

echo ""
echo "========================================"
echo "  Backend  : http://localhost:4000"
echo "  Frontend : http://localhost:5173"
echo "  Swagger  : http://localhost:4000/swagger-ui/"
echo "========================================"
echo "  종료하려면 Ctrl+C 를 누르세요."
echo "========================================"
echo ""

wait $BACKEND_PID $FRONTEND_PID
