#!/usr/bin/env bash
# ==============================================================================
# Rust P2P Workspace 자동화 파이프라인 스크립트 (Linux / macOS / WSL / Git Bash)
# 포맷팅(fmt) -> 린팅(clippy) -> 단위테스트(test) -> 빌드(build) -> 앱 실행(run)
#
# 옵션:
#   --skip-checks : fmt, clippy, test 건너뛰기
#   --skip-run    : 앱 실행 건너뛰기 (빌드/검증만 수행)
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$WORKSPACE_ROOT"

# ANSI 컬러 코드
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${CYAN}==========================================================${NC}"
echo -e "${CYAN} [Rust P2P Chat] 파이프라인 자동화 스크립트${NC}"
echo -e " 루트 경로: ${WORKSPACE_ROOT}"
echo -e "${CYAN}==========================================================${NC}"

# 인자 파싱
SKIP_CHECKS=false
SKIP_RUN=false

for arg in "$@"; do
    case "$arg" in
        --skip-checks)
            SKIP_CHECKS=true
            ;;
        --skip-run)
            SKIP_RUN=true
            ;;
        *)
            echo -e "${RED}[!] 알 수 없는 옵션: $arg${NC}"
            echo "사용법: ./scripts/run.sh [--skip-checks] [--skip-run]"
            exit 1
            ;;
    esac
done

# 0. cargo 명령 확인
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}[!] cargo 명령어를 찾을 수 없습니다. Rust 환경을 확인하세요.${NC}"
    exit 1
fi

if [ "$SKIP_CHECKS" = false ]; then
    # 1. 포맷팅
    echo -e "\n${YELLOW}[1/5] 코드 포맷팅 검사 및 적용 (cargo fmt)...${NC}"
    cargo fmt --all
    echo -e "${GREEN}  -> 코드 포맷팅 완료${NC}"

    # 2. 린팅
    echo -e "\n${YELLOW}[2/5] 정적 분석 및 린팅 (cargo clippy)...${NC}"
    cargo clippy --workspace --all-targets -- -D warnings
    echo -e "${GREEN}  -> 린팅 통과 (경고 0건)${NC}"

    # 3. 테스트
    echo -e "\n${YELLOW}[3/5] 테스트 실행 (cargo test)...${NC}"
    cargo test --workspace
    echo -e "${GREEN}  -> 모든 테스트 통과${NC}"
else
    echo -e "\n[*] --skip-checks 옵션으로 포맷팅/린팅/테스트를 건너뜁니다."
fi

# 4. 빌드
echo -e "\n${YELLOW}[4/5] 전체 워크스페이스 빌드 (cargo build)...${NC}"
cargo build --workspace
echo -e "${GREEN}  -> 바이너리 빌드 성공${NC}"

# 5. 앱 실행 (창을 닫으면 종료)
if [ "$SKIP_RUN" = false ]; then
    echo -e "\n${YELLOW}[5/5] P2P Chat 앱 실행 (창을 닫으면 종료됩니다)...${NC}"
    cargo run -p p2p_gui
else
    echo -e "\n[*] --skip-run 옵션으로 앱 실행을 건너뜁니다."
fi
