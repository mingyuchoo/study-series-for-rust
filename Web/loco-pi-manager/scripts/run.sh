#!/usr/bin/env bash
set -euo pipefail

# 설정, 템플릿, .env의 상대 경로는 항상 프로젝트 루트를 기준으로 한다.
PROJECT_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd -- "$PROJECT_ROOT"

usage() {
    cat <<'EOF'
사용법: ./scripts/run.sh <명령> [옵션]

명령:
  fmt [옵션]          워크스페이스 코드 포맷팅 (검사만: --check)
  build [옵션]        워크스페이스 빌드 (최적화: --release)
  test [옵션]         임시 SQLite DB로 워크스페이스 테스트
  run [--release] [Loco 옵션]
                      서버 실행 (기본 포트: 5150, 종료: Ctrl+C)
  all [--release]     포맷팅 → 빌드 → 테스트 → 서버 실행
  help                도움말 (명령 생략 시에도 표시)

fmt/build/test의 추가 옵션은 해당 Cargo 명령으로 전달됩니다.
run의 --release는 첫 번째 옵션으로 지정하고, 나머지는 Loco start에 전달합니다.
all은 --release만 지원하며, 어느 단계든 실패하면 즉시 중단합니다.
서버 환경은 앱의 .env, LOCO_ENV 또는 Loco --environment 옵션으로 설정합니다.
test는 LOCO_ENV=test 및 별도의 DATABASE_URL을 사용하고 종료 시 DB를 삭제합니다.

예시:
  ./scripts/run.sh fmt --check
  ./scripts/run.sh build --release
  ./scripts/run.sh test can_find_by_email -- --nocapture
  ./scripts/run.sh run
  ./scripts/run.sh run --release --environment production
  ./scripts/run.sh all
EOF
}

fail() {
    printf '오류: %s\n' "$*" >&2
    exit 1
}

format_code() {
    printf '\n==> 코드 포맷팅\n'
    cargo fmt --all "$@"
}

build_project() {
    printf '\n==> 워크스페이스 빌드\n'
    cargo build --workspace "$@"
}

test_project() (
    # test.yaml은 DB를 재생성하므로 기존 DATABASE_URL을 상속하지 않는다.
    test_dir="$(mktemp -d "${TMPDIR:-/tmp}/pi-manager-test.XXXXXX")"
    trap 'rm -rf -- "$test_dir"' EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    export LOCO_ENV=test
    export DATABASE_URL="sqlite://${test_dir}/test.sqlite?mode=rwc"

    printf '\n==> 테스트 (임시 SQLite DB)\n'
    cargo test --workspace "$@"
)

run_server() {
    printf '\n==> PI Manager 서버 시작\n'
    # exec로 서버 실행 프로세스에 종료 신호와 종료 코드를 전달한다.
    if [[ "${1:-}" == "--release" ]]; then
        shift
        exec cargo run --release --bin pi_manager-cli -- start "$@"
    else
        exec cargo run --bin pi_manager-cli -- start "$@"
    fi
}

action="${1:-help}"
if [[ $# -gt 0 ]]; then
    shift
fi

# 도움말/입력 오류는 Rust 도구 설치 여부와 관계없이 확인할 수 있다.
case "$action" in
    help|-h|--help) usage; exit 0 ;;
    fmt|build|test|run) ;;
    all)
        if [[ $# -gt 1 || ($# -eq 1 && "$1" != "--release") ]]; then
            fail 'all 명령에는 --release만 지정할 수 있습니다.'
        fi
        ;;
    *) usage >&2; fail "알 수 없는 명령: $action" ;;
esac

command -v cargo >/dev/null 2>&1 || fail 'Cargo가 없습니다. Rust 툴체인을 설치하세요.'
if [[ "$action" == fmt || "$action" == all ]]; then
    cargo fmt --version >/dev/null 2>&1 || fail 'rustfmt가 필요합니다: rustup component add rustfmt'
fi

case "$action" in
    fmt) format_code "$@" ;;
    build) build_project "$@" ;;
    test) test_project "$@" ;;
    run) run_server "$@" ;;
    all)
        format_code
        build_project "$@"
        test_project "$@"
        run_server "$@"
        ;;
esac
