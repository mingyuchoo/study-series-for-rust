#requires -Version 7.4
# Release pipeline: formatting intentionally updates source files.
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false

function Show-ReleaseUsage {
    @'
Usage: pwsh -File scripts/release.ps1 [--target TARGET] [--help]

Format, lint, test, release-build, bundle ZIP, installer EXE, SHA256 checksums.
Windows only; requires Rust rustfmt/clippy and Inno Setup 6.3+.
TARGET: x86_64-pc-windows-msvc or aarch64-pc-windows-msvc (default: rustc host).
ISCC_PATH can specify the Inno Setup compiler. No GUI is launched.
'@
}

$releaseTarget = ''
for ($index = 0; $index -lt $args.Count; $index++) {
    switch -CaseSensitive ($args[$index]) {
        '--help' { Show-ReleaseUsage; exit 0 }
        '--target' {
            if (++$index -ge $args.Count) {
                [Console]::Error.WriteLine('--target requires a target triple.'); exit 2
            }
            $releaseTarget = $args[$index]
        }
        default {
            [Console]::Error.WriteLine("Unknown script option: $($args[$index])")
            Show-ReleaseUsage; exit 2
        }
    }
}

try {
    . (Join-Path $PSScriptRoot '_common.ps1')
    $PSNativeCommandUseErrorActionPreference = $false
    if (-not $IsWindows) { Stop-Verification 'Release packaging requires native Windows PowerShell.' }
    $supportedTargets = @('x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')
    if ($releaseTarget -and $releaseTarget -notin $supportedTargets) {
        Stop-Verification "Unsupported release target: $releaseTarget"
    }
    foreach ($tool in @('cargo', 'rustc', 'rustup')) {
        if (-not (Get-Command $tool -CommandType Application -ErrorAction SilentlyContinue)) {
            Stop-Verification "Missing required tool: $tool"
        }
    }

    function Invoke-ReleaseCommand {
        param([string]$Command, [string[]]$Arguments)
        $global:LASTEXITCODE = 0
        & $Command @Arguments
        if ($LASTEXITCODE -ne 0) {
            Stop-Verification "FAIL: $Command (exit $LASTEXITCODE). Remaining stages not run." $LASTEXITCODE
        }
    }

    $rustInfo = Invoke-ReleaseCommand 'rustc' @('-vV')
    $buildHost = ($rustInfo | Select-String '^host: (.+)$').Matches.Groups[1].Value
    if ($buildHost -notin $supportedTargets) { Stop-Verification "Unsupported rustc host: $buildHost" }
    if (-not $releaseTarget) { $releaseTarget = $buildHost }
    $installedTargets = Invoke-ReleaseCommand 'rustup' @('target', 'list', '--installed')
    if ($releaseTarget -notin $installedTargets) {
        Stop-Verification "Missing Rust target. Run: rustup target add $releaseTarget"
    }
    Invoke-ReleaseCommand 'cargo' @('fmt', '--version') | Out-Null
    Invoke-ReleaseCommand 'cargo' @('clippy', '--version') | Out-Null

    $iscc = $env:ISCC_PATH
    if (-not $iscc) {
        $found = Get-Command 'ISCC.exe' -CommandType Application -ErrorAction SilentlyContinue
        if ($found) { $iscc = $found.Source }
        else {
            $iscc = @(
                "$env:LOCALAPPDATA/Programs/Inno Setup 6/ISCC.exe",
                "${env:ProgramFiles(x86)}/Inno Setup 6/ISCC.exe",
                "$env:ProgramFiles/Inno Setup 6/ISCC.exe"
            ) | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
        }
    }
    if (-not $iscc -or -not (Test-Path -LiteralPath $iscc -PathType Leaf)) {
        Stop-Verification 'Missing Inno Setup 6.3+ compiler. Install Inno Setup or set ISCC_PATH.'
    }
    $metadata = (Invoke-ReleaseCommand 'cargo' @('metadata', '--locked', '--no-deps', '--format-version', '1') | Out-String) | ConvertFrom-Json
    $manifest = Join-Path $RepositoryRoot 'Cargo.toml'
    $package = @($metadata.packages | Where-Object { [IO.Path]::GetFullPath($_.manifest_path) -eq $manifest })
    if ($package.Count -ne 1) { Stop-Verification 'Cargo metadata did not identify the application package.' }
    $appName = $package[0].name
    $appVersion = $package[0].version
    if ($appName -notmatch '^[a-zA-Z0-9_-]+$' -or $appVersion -notmatch '^[0-9A-Za-z.+-]+$') {
        Stop-Verification 'Unsafe application name/version in Cargo metadata.'
    }
    $arch = if ($releaseTarget -eq 'aarch64-pc-windows-msvc') { 'arm64' } else { 'x64' }
    $stem = "$appName-$appVersion-$arch"

    Write-Output "`n== format =="
    Invoke-ReleaseCommand (Join-Path $ScriptDirectory 'format.ps1') @('--write')
    Write-Output "`n== lint =="
    Invoke-ReleaseCommand 'cargo' @('clippy', '--locked', '--all-targets', '--features', 'test-support', '--target', $buildHost, '--', '-D', 'warnings')
    Write-Output "`n== test =="
    Invoke-ReleaseCommand 'cargo' @('test', '--locked', '--all-targets', '--features', 'test-support', '--target', $buildHost)
    Write-Output "`n== doctest =="
    Invoke-ReleaseCommand 'cargo' @('test', '--locked', '--doc', '--features', 'test-support', '--target', $buildHost)
    Write-Output "`n== build =="
    Invoke-ReleaseCommand 'cargo' @('build', '--release', '--locked', '--target', $releaseTarget)

    $binary = Join-Path $metadata.target_directory "$releaseTarget/release/$appName.exe"
    if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) { Stop-Verification "Missing release binary: $binary" }
    $output = Join-Path $RepositoryRoot ".artifacts/releases/$stem-$([DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfff'))-$([guid]::NewGuid().ToString('N'))"
    $bundle = Join-Path $output 'bundle'
    $licenses = Join-Path $bundle 'licenses'
    New-Item -ItemType Directory -Path $licenses | Out-Null
    Write-Output "`n== bundle =="
    Copy-Item -LiteralPath $binary -Destination (Join-Path $bundle "$appName.exe")
    Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'README.md') -Destination $bundle
    Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'assets/GPUI-LICENSE-APACHE') -Destination $licenses
    Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'assets/fonts/LICENSE-Pretendard.txt') -Destination $licenses
    $zip = Join-Path $output "$stem.zip"
    Compress-Archive -Path (Join-Path $bundle '*') -DestinationPath $zip

    Write-Output "`n== installer =="
    $installerStem = "$stem-setup"
    Invoke-ReleaseCommand $iscc @("/DBundleDir=$bundle", "/DOutputDir=$output", "/DOutputBaseFilename=$installerStem", "/DAppName=$appName", "/DAppVersion=$appVersion", "/DAppArch=$arch", (Join-Path $RepositoryRoot 'packaging/stillnote.iss'))
    $installer = Join-Path $output "$installerStem.exe"
    if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) { Stop-Verification "Missing installer: $installer" }
    Write-Output "`n== checksum =="
    @($zip, $installer) | ForEach-Object {
        "$((Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant())  $([IO.Path]::GetFileName($_))"
    } | Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt') -Encoding utf8NoBOM
    Write-Output "Release complete: $output"
    exit 0
}
catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    if (Get-Command Get-VerificationExitCode -ErrorAction SilentlyContinue) {
        exit (Get-VerificationExitCode $_)
    }
    exit 1
}
