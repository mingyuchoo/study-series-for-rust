#Requires -Version 5.1
<#
.SYNOPSIS
Clean, build, test, and run presentation on Windows.
.EXAMPLE
.\scripts\run.ps1
.EXAMPLE
.\scripts\run.ps1 --example-argument "value with spaces"
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
# Handle native exit codes explicitly in Windows PowerShell and PowerShell 7.
$PSNativeCommandUseErrorActionPreference = $false

$applicationArguments = @($args)
$projectRoot = Split-Path -Parent $PSScriptRoot
$cargoCommand = Get-Command cargo -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
if ($null -eq $cargoCommand) {
    Write-Error 'Cargo was not found. Install Rust with rustup and ensure Cargo is on PATH.' -ErrorAction Continue
    exit 1
}

$steps = @(
    @{ Message = 'Cleaning project'; Arguments = @('clean') }
    @{ Message = 'Building presentation package'; Arguments = @('build', '--package', 'presentation') }
    @{ Message = 'Running workspace tests'; Arguments = @('test', '--workspace') }
    @{ Message = 'Starting presentation app'; Arguments = @('run', '--package', 'presentation', '--') + $applicationArguments }
)

$exitCode = 0
Push-Location -LiteralPath $projectRoot
try {
    if ([Environment]::UserInteractive -and -not [Console]::IsOutputRedirected) {
        try {
            Clear-Host
        }
        catch {
            # Ignore clear failure if host does not support it
        }
    }

    foreach ($step in $steps) {
        Write-Host "==> $($step.Message)"
        $cargoArguments = $step.Arguments
        & $cargoCommand.Path @cargoArguments
        if ($LASTEXITCODE -ne 0) {
            $exitCode = $LASTEXITCODE
            break
        }
    }
}
catch {
    Write-Error $_ -ErrorAction Continue
    $exitCode = 1
}
finally {
    Pop-Location
}

exit $exitCode
