#Requires -Version 5.1
<#
.SYNOPSIS
    ratatui-diary 워크스페이스를 빌드 -> 테스트 -> 실행 순으로 순차 수행한다.

.DESCRIPTION
    각 단계를 순서대로 실행하며, 앞 단계가 실패하면 즉시 중단한다.
      1. Build : cargo build --workspace
      2. Test  : cargo test  --workspace
      3. Run   : cargo run   (기본 멤버: ratatui-diary)

.PARAMETER Release
    release 프로파일로 빌드/실행한다. (기본: dev)

.PARAMETER SkipTest
    테스트 단계를 건너뛴다.

.PARAMETER NoRun
    실행 단계를 건너뛰고 빌드/테스트만 수행한다.

.EXAMPLE
    ./scripts/run.ps1

.EXAMPLE
    ./scripts/run.ps1 -Release

.EXAMPLE
    ./scripts/run.ps1 -SkipTest
#>
[CmdletBinding()]
param(
    [switch] $Release,
    [switch] $SkipTest,
    [switch] $NoRun
)

$ErrorActionPreference = 'Stop'

# 스크립트 위치를 기준으로 프로젝트 루트(scripts/의 상위)로 이동한다.
$ProjectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $ProjectRoot

function Invoke-Step {
    param(
        [Parameter(Mandatory)] [string]   $Name,
        [Parameter(Mandatory)] [string[]] $CargoArgs
    )

    Write-Host ""
    Write-Host "==> $Name : cargo $($CargoArgs -join ' ')" -ForegroundColor Cyan

    & cargo @CargoArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Host "!! $Name 단계 실패 (exit code: $LASTEXITCODE)" -ForegroundColor Red
        Pop-Location
        exit $LASTEXITCODE
    }
    Write-Host "==> $Name 완료" -ForegroundColor Green
}

try {
    # cargo 설치 여부 확인
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Host "!! cargo를 찾을 수 없습니다. Rust 툴체인을 설치하세요: https://rustup.rs" -ForegroundColor Red
        exit 1
    }

    $profileArgs = if ($Release) { @('--release') } else { @() }

    # 1) 빌드
    Invoke-Step -Name 'Build' -CargoArgs (@('build', '--workspace') + $profileArgs)

    # 2) 테스트
    if (-not $SkipTest) {
        Invoke-Step -Name 'Test' -CargoArgs (@('test', '--workspace') + $profileArgs)
    }
    else {
        Write-Host ""
        Write-Host "==> Test 단계 건너뜀 (-SkipTest)" -ForegroundColor Yellow
    }

    # 3) 실행 (기본 멤버: ratatui-diary)
    if (-not $NoRun) {
        Invoke-Step -Name 'Run' -CargoArgs (@('run') + $profileArgs)
    }
    else {
        Write-Host ""
        Write-Host "==> Run 단계 건너뜀 (-NoRun)" -ForegroundColor Yellow
    }
}
finally {
    Pop-Location
}
