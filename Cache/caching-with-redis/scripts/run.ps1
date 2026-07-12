#!/usr/bin/env pwsh
# 모든 출력/주석은 한국어
param(
    [Parameter(Position = 0)] [string]$Command = "help",
    [Parameter(Position = 1)] [string]$Arg1
)

$ErrorActionPreference = "Stop"

function Write-Info($msg) { Write-Host "ℹ️  $msg" -ForegroundColor Blue }
function Write-Ok($msg) { Write-Host "✅ $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "⚠️  $msg" -ForegroundColor Yellow }
function Write-Err($msg) { Write-Host "❌ $msg" -ForegroundColor Red }

function Check-Rust {
    if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) {
        Write-Warn "Rust가 설치되어 있지 않습니다. https://rustup.rs 에서 설치 후 다시 시도하세요."
        throw "Rust 미설치"
    }
    Write-Ok ("Rust 버전: " + (rustc --version))
}

function Check-Docker {
    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
        Write-Err "Docker가 설치되어 있지 않습니다. 설치 후 다시 시도하세요."
        throw "Docker 미설치"
    }
    Write-Ok ("Docker 버전: " + (docker --version))
}

function Cargo-Build {
    Write-Info "Rust 프로젝트 빌드(Release) 수행"
    cargo build --release
    Write-Ok "빌드 완료: target/release/caching_with_redis(.exe)"
}

function Docker-Build {
    Write-Info "Docker 이미지 빌드"
    docker build -t caching_with_redis:latest .
    Write-Ok "이미지 빌드 완료: caching_with_redis:latest"
}

function Docker-Compose-Up {
    Write-Info "docker compose up -d --build"
    docker compose up -d --build
    Write-Ok "Compose 기동 완료"
}

function Docker-Compose-Down {
    Write-Info "docker compose down -v"
    docker compose down -v
    Write-Ok "Compose 정리 완료"
}

function Docker-Compose-Cmd {
    param([string]$Subcmd, [string[]]$Rest)
    docker compose run --rm embedding-cache $Subcmd @Rest
}

function Docker-Run-Once {
    $redisUrl = if ($env:REDIS_URL) { $env:REDIS_URL } else { 'redis://host.docker.internal:6379' }
    Write-Info "단일 컨테이너 실행(REDIS_URL=$redisUrl)"
    docker run --rm `
        -e REDIS_URL=$redisUrl `
        -e CACHE_TTL=$($env:CACHE_TTL ?? 3600) `
        -e RUST_LOG=$($env:RUST_LOG ?? 'info') `
        caching_with_redis:latest status
}

function Print-Help {
    @"
🦀 caching_with_redis 설치/실행 스크립트 (PowerShell)
사용법: .\scripts\setup.ps1 <명령> [옵션]

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
"@
}

try {
    switch ($Command) {
        "check" { Check-Rust; Check-Docker }
        "build" { Cargo-Build }
        "image" { Docker-Build }
        "up" { Docker-Compose-Up }
        "down" { Docker-Compose-Down }
        "run-once" { Docker-Run-Once }
        "status" { Docker-Compose-Cmd -Subcmd "status" }
        "test" { Docker-Compose-Cmd -Subcmd "test" -Rest @("--text", ($Arg1 ?? "안녕하세요!")) }
        "stats" { Docker-Compose-Cmd -Subcmd "stats" }
        "clear" { Docker-Compose-Cmd -Subcmd "clear" }
        default { Print-Help }
    }
}
catch {
    Write-Err $_
    exit 1
}
