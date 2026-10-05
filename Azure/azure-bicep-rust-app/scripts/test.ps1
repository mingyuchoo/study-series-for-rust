#!/usr/bin/env pwsh
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'common.ps1')
try {
    Write-Host 'Testing SolidJS + Rust application...' -ForegroundColor Green
    Invoke-InDirectory $FrontendDirectory {
        Invoke-Pnpm install --frozen-lockfile
        Invoke-Pnpm run lint
        Invoke-Pnpm test
    }
    Invoke-InDirectory $BackendDirectory { Invoke-Cargo test --locked }
    Write-Host 'All tests passed!' -ForegroundColor Green
} catch {
    Write-Error $_ -ErrorAction Continue
    exit 1
}
