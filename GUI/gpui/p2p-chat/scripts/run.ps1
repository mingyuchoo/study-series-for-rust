<#
.SYNOPSIS
    Rust P2P Workspace 자동화 파이프라인 스크립트
    포맷팅(fmt) -> 린팅(clippy) -> 테스트(test) -> 빌드(build) -> 앱 실행(run)
.PARAMETER SkipChecks
    포맷팅, 린팅, 테스트를 건너뛰고 빌드 및 실행만 수행
.PARAMETER SkipRun
    검증(fmt, clippy, test, build)만 수행하고 앱 실행은 건너뜀
#>

[CmdletBinding()]
param (
    [switch]$SkipChecks,
    [switch]$SkipRun
)

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$WorkspaceRoot = (Resolve-Path (Join-Path $ScriptDir "..")).Path

Set-Location $WorkspaceRoot

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host " [Rust P2P Chat] 파이프라인 자동화 스크립트" -ForegroundColor Cyan
Write-Host " 루트 경로: $WorkspaceRoot" -ForegroundColor DarkGray
Write-Host "==========================================================" -ForegroundColor Cyan

# 0. 도구 가용성 검사
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "[!] 'cargo' 명령을 찾을 수 없습니다. Rust 환경 변수(PATH)를 확인해주세요." -ForegroundColor Red
    exit 1
}

function Run-Step {
    param (
        [string]$StepName,
        [scriptblock]$CommandBlock
    )
    Write-Host "`n$StepName" -ForegroundColor Yellow
    & $CommandBlock
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[-] $StepName 실패 (종료 코드: $LASTEXITCODE)" -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

if (-not $SkipChecks) {
    # 1. 코드 포맷팅 (cargo fmt)
    Run-Step "[1/5] 코드 포맷팅 검사 및 적용 (cargo fmt)..." {
        cargo fmt --all
    }
    Write-Host "  -> 코드 포맷팅 완료" -ForegroundColor Green

    # 2. 린팅 (cargo clippy)
    Run-Step "[2/5] 정적 분석 및 린팅 (cargo clippy)..." {
        cargo clippy --workspace --all-targets -- -D warnings
    }
    Write-Host "  -> 린팅 통과 (경고 0건)" -ForegroundColor Green

    # 3. 단위 테스트 (cargo test)
    Run-Step "[3/5] 테스트 실행 (cargo test)..." {
        cargo test --workspace
    }
    Write-Host "  -> 모든 테스트 통과" -ForegroundColor Green
} else {
    Write-Host "`n[*] -SkipChecks 플래그에 의해 포맷팅/린팅/테스트를 건너뜁니다." -ForegroundColor DarkGray
}

# 4. 빌드 (cargo build)
Run-Step "[4/5] 전체 워크스페이스 빌드 (cargo build)..." {
    cargo build --workspace
}
Write-Host "  -> 바이너리 빌드 성공" -ForegroundColor Green

# 5. 앱 실행 (P2P Chat GUI, 창을 닫으면 종료)
if (-not $SkipRun) {
    Run-Step "[5/5] P2P Chat 앱 실행 (창을 닫으면 종료됩니다)..." {
        cargo run -p p2p_gui
    }
} else {
    Write-Host "`n[*] -SkipRun 플래그에 의해 앱 실행을 건너뜁니다." -ForegroundColor DarkGray
}
