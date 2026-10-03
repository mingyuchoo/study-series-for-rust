#Requires -Version 5.1
<#
.SYNOPSIS
Format, lint, build, test, bundle, and create a Windows installer.
.EXAMPLE
.\scripts\release.ps1 -CheckFormatting
.EXAMPLE
.\scripts\release.ps1 -IsccPath 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe'
#>
[CmdletBinding()]
param(
    [switch]$CheckFormatting,
    [string]$IsccPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$projectRoot = Split-Path -Parent $PSScriptRoot

function Invoke-Native {
    param([string]$Command, [string[]]$Arguments)
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Command failed with exit code $LASTEXITCODE."
    }
}

$exitCode = 0
$stage = $null
Push-Location -LiteralPath $projectRoot
try {
    if ($env:OS -ne 'Windows_NT') { throw 'Use scripts/release.sh on macOS/Linux.' }
    foreach ($tool in @('cargo', 'rustc')) {
        if (-not (Get-Command $tool -CommandType Application -ErrorAction SilentlyContinue)) {
            throw "$tool was not found. Install Rust using rustup and add it to PATH."
        }
    }
    if (-not $IsccPath) {
        $compiler = Get-Command ISCC.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($compiler) { $IsccPath = $compiler.Path }
        else {
            foreach ($base in @(${env:ProgramFiles(x86)}, $env:ProgramFiles)) {
                if ($base) {
                    $candidate = Join-Path $base 'Inno Setup 6\ISCC.exe'
                    if (Test-Path -LiteralPath $candidate -PathType Leaf) { $IsccPath = $candidate; break }
                }
            }
        }
    }
    if (-not $IsccPath -or -not (Test-Path -LiteralPath $IsccPath -PathType Leaf)) {
        throw 'Install Inno Setup 6 (https://jrsoftware.org/isinfo.php), add ISCC.exe to PATH, or pass -IsccPath.'
    }
    $IsccPath = (Resolve-Path -LiteralPath $IsccPath).Path
    $rustInfo = & rustc -vV
    if ($LASTEXITCODE -ne 0) { throw 'rustc -vV failed.' }
    $target = ($rustInfo | Where-Object { $_ -like 'host: *' }) -replace '^host: ', ''
    switch ($target) {
        'x86_64-pc-windows-msvc' { $architecture = 'ArchitecturesAllowed=x64compatible'; $installMode = 'ArchitecturesInstallIn64BitMode=x64compatible' }
        'i686-pc-windows-msvc' { $architecture = ''; $installMode = '' }
        default { throw "Unsupported Windows host: $target. Use an x86/x64 MSVC Rust toolchain." }
    }
    $metadataJson = & cargo metadata --no-deps --format-version 1 --locked
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed.' }
    $metadata = ($metadataJson -join "`n") | ConvertFrom-Json
    $package = $metadata.packages | Where-Object { $_.name -eq 'window-app' } | Select-Object -First 1
    if (-not $package) { throw 'window-app package was not found.' }
    $version = $package.version
    if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'Installer requires a numeric major.minor.patch package version.' }

    Write-Host '==> Formatting Rust code'
    $formatArgs = @('fmt', '--all')
    if ($CheckFormatting) { $formatArgs += @('--', '--check') }
    Invoke-Native cargo $formatArgs
    Write-Host '==> Linting release targets'
    Invoke-Native cargo @('clippy', '--workspace', '--all-targets', '--release', '--locked', '--target', $target, '--', '-D', 'warnings')
    Write-Host '==> Building release targets'
    Invoke-Native cargo @('build', '--workspace', '--all-targets', '--release', '--locked', '--target', $target)
    Write-Host '==> Testing release targets'
    Invoke-Native cargo @('test', '--workspace', '--release', '--locked', '--target', $target)

    $releaseDir = Join-Path $metadata.target_directory "$target\release"
    $binary = Join-Path $releaseDir 'window-app.exe'
    if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) { throw "Missing release executable: $binary" }
    $dist = Join-Path $projectRoot "dist\$version\$target"
    New-Item -ItemType Directory -Path $dist -Force | Out-Null
    $stage = Join-Path $dist ('.stage-' + [guid]::NewGuid().ToString('N'))
    $bundle = Join-Path $stage 'window-app'
    New-Item -ItemType Directory -Path $bundle -Force | Out-Null
    Copy-Item -LiteralPath $binary -Destination $bundle
    Get-ChildItem -LiteralPath $releaseDir -Filter '*.dll' -File | Copy-Item -Destination $bundle
    Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination $bundle
    $artifactName = "window-app-$version-$target"
    Write-Host '==> Creating portable ZIP'
    $zip = Join-Path $dist "$artifactName.zip"
    Compress-Archive -LiteralPath $bundle -DestinationPath $zip -Force

    Write-Host '==> Creating installer'
    $installerSource = Join-Path $stage 'installer.iss'
    # UTF-8 BOM is required for reliable Unicode paths with older Inno versions.
    $definition = @"
[Setup]
AppId=mingyuchoo.window-app
AppName=window-app
AppVersion=$version
DefaultDirName={localappdata}\Programs\window-app
DefaultGroupName=window-app
PrivilegesRequired=lowest
OutputDir=$dist
OutputBaseFilename=$artifactName-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\window-app.exe
$architecture
$installMode

[Files]
Source: "$bundle\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\window-app"; Filename: "{app}\window-app.exe"

[Run]
Filename: "{app}\window-app.exe"; Description: "Launch window-app"; Flags: nowait postinstall skipifsilent
"@
    [System.IO.File]::WriteAllText($installerSource, $definition, (New-Object System.Text.UTF8Encoding($true)))
    Invoke-Native $IsccPath @($installerSource)
    $installer = Join-Path $dist "$artifactName-setup.exe"
    if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) { throw 'Installer compiler produced no setup executable.' }
    $hashLines = foreach ($artifact in @($zip, $installer)) {
        $hash = Get-FileHash -LiteralPath $artifact -Algorithm SHA256
        "$($hash.Hash.ToLowerInvariant())  $([System.IO.Path]::GetFileName($artifact))"
    }
    [System.IO.File]::WriteAllLines((Join-Path $dist 'SHA256SUMS'), [string[]]$hashLines, (New-Object System.Text.UTF8Encoding($false)))
    Write-Host "==> Release artifacts: $dist"
}
catch {
    Write-Error $_ -ErrorAction Continue
    $exitCode = 1
}
finally {
    # Remove only this invocation's staging directory, after checking its parent.
    if ($stage -and (Test-Path -LiteralPath $stage)) {
        $resolvedStage = (Resolve-Path -LiteralPath $stage).Path
        if ((Split-Path -Parent $resolvedStage) -eq $dist -and (Split-Path -Leaf $resolvedStage) -like '.stage-*') {
            Remove-Item -LiteralPath $resolvedStage -Recurse -Force
        }
    }
    Pop-Location
}
exit $exitCode
