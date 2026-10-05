#!/usr/bin/env pwsh
[CmdletBinding()]
param([switch]$Run)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'common.ps1')

try {
    Write-Host 'Building integrated SolidJS + Rust application...' -ForegroundColor Green
    Build-Frontend
    Invoke-InDirectory $BackendDirectory { Invoke-Cargo build --locked }
    Write-Host 'Build completed successfully!' -ForegroundColor Green
    if ($Run) {
        Write-Host 'Application: http://localhost:8000'
        Write-Host 'Swagger UI: http://localhost:8000/swagger-ui/'
        Invoke-InDirectory $BackendDirectory { Invoke-Cargo run --locked }
    }
} catch {
    Write-Error $_ -ErrorAction Continue
    exit 1
}
