#!/usr/bin/env pwsh

<#
.SYNOPSIS
  Builds, tests, and bundles the app in release mode, producing Windows installers.

.DESCRIPTION
  Windows counterpart of scripts/release.sh. Tauri bundles cannot be
  cross-compiled, so this must run on Windows and produces:
    - .exe  (NSIS setup)     via the `nsis` bundle
    - .msi  (WiX installer)  via the `msi` bundle
  Tauri downloads NSIS and the WiX Toolset automatically on first build, so no
  extra toolchain is required beyond cargo, dx, and cargo tauri.

  To produce every platform's installer from a single trigger, push an
  `ikik-v*` tag: the repo-root CI runs this on Windows and release.sh on
  macOS/Debian/Fedora, attaching all installers to the GitHub Release.

.PARAMETER Bundles
  Comma-separated bundle types to build. Default: nsis,msi.

.PARAMETER NoClean
  Skip `cargo clean` (faster, but less reproducible).

.PARAMETER SkipTests
  Skip the test suite.

.EXAMPLE
  scripts/release.ps1
  scripts/release.ps1 -Bundles msi -SkipTests
#>

[CmdletBinding()]
param(
    [string]$Bundles = "nsis,msi",
    [switch]$NoClean,
    [switch]$SkipTests,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

function Write-Log {
    param([string]$Message)
    Write-Host "`n==> $Message"
}

function Show-Usage {
    Write-Host @"
Usage: scripts/release.ps1 [options]

Options:
  -Bundles <list>  Comma-separated bundle types to build (e.g. nsis,msi).
                   nsis -> .exe, msi -> .msi. Default: nsis,msi.
  -NoClean         Skip ``cargo clean`` (faster, but less reproducible).
  -SkipTests       Skip the test suite.
  -Help            Show this help.
"@
}

function Require-Command {
    param([string]$Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "required command not found: $Name"
    }
}

function Assert-LastExit {
    param([string]$What)
    if ($LASTEXITCODE -ne 0) {
        throw "$What failed (exit code $LASTEXITCODE)"
    }
}

if ($Help) {
    Show-Usage
    exit 0
}

# Tauri Windows installers (.exe/.msi) can only be built on Windows.
if (-not ($IsWindows -or $env:OS -eq "Windows_NT")) {
    throw "release.ps1 builds Windows installers and must run on Windows. Use scripts/release.sh on macOS/Linux."
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = (Resolve-Path (Join-Path $ScriptDir "..")).Path
Set-Location $RootDir

$BundleList = $Bundles.Split(",") | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne "" }
if ($BundleList.Count -eq 0) {
    throw "-Bundles requires at least one value"
}

# --- Verify toolchain -------------------------------------------------------
Require-Command cargo
Require-Command dx

& cargo tauri --version *> $null
if ($LASTEXITCODE -ne 0) {
    Write-Host "error: required cargo subcommand not found: cargo tauri" -ForegroundColor Red
    Write-Host "hint: install it with: cargo install tauri-cli"
    exit 1
}

Write-Log "Host: Windows $env:PROCESSOR_ARCHITECTURE | bundles: $($BundleList -join ',')"

# --- Clean ------------------------------------------------------------------
if (-not $NoClean) {
    Write-Log "Clean build artifacts"
    cargo clean
    Assert-LastExit "cargo clean"
}

# --- Test -------------------------------------------------------------------
if (-not $SkipTests) {
    Write-Log "Run workspace tests"
    cargo test --workspace
    Assert-LastExit "cargo test"
}

# --- Build & bundle ---------------------------------------------------------
# Run from the crate that owns tauri.conf.json so the CLI resolves the config
# and its relative beforeBuildCommand / frontendDist paths correctly.
Write-Log "Build and bundle (release)"
Set-Location (Join-Path $RootDir "presentation_backend")
cargo tauri build --bundles $BundleList
Assert-LastExit "cargo tauri build"

# --- Report -----------------------------------------------------------------
$BundleDir = Join-Path $RootDir "target/release/bundle"
Write-Log "Done. Installers written to:"
if (Test-Path $BundleDir) {
    Get-ChildItem -Path $BundleDir -Recurse -File -Include *.exe, *.msi |
        ForEach-Object { Write-Host $_.FullName }
} else {
    Write-Warning "bundle directory not found: $BundleDir"
}
