#requires -Version 7.4
# Development pipeline: formatting intentionally updates source files.
$ErrorActionPreference = 'Stop'

function Show-RunUsage {
    @'
Usage: pwsh -File scripts/run.ps1 [--no-run] [--help] [-- APP_ARGUMENTS...]

Format, lint, test all targets and doctests, build, then run Stillnote.
  --no-run  Complete all checks and the build without opening the GUI.
  --help    Show this help without running any commands.
  --        Pass the remaining arguments unchanged to Stillnote.

Requires PowerShell 7.4+ and Rust stable with rustfmt/clippy.
'@
}

$noRun = $false
$appArguments = @()
for ($index = 0; $index -lt $args.Count; $index++) {
    switch -CaseSensitive ($args[$index]) {
        '--no-run' { $noRun = $true }
        '--help' { Show-RunUsage; exit 0 }
        '--' {
            if ($index + 1 -lt $args.Count) {
                $appArguments = @($args[($index + 1)..($args.Count - 1)])
            }
            $index = $args.Count
        }
        default {
            [Console]::Error.WriteLine("Unknown script option: $($args[$index])")
            Show-RunUsage
            exit 2
        }
    }
}

try {
    . (Join-Path $PSScriptRoot '_common.ps1')
    # Explicit exit-code checks also cover child PowerShell script exits.
    $PSNativeCommandUseErrorActionPreference = $false

    function Invoke-RunStage {
        param([string]$Stage, [string]$Command, [string[]]$Arguments)
        Write-Output "`n== $Stage =="
        $global:LASTEXITCODE = 0
        & $Command @Arguments
        if ($LASTEXITCODE -ne 0) {
            Stop-Verification "FAIL: $Stage (exit $LASTEXITCODE). Remaining stages not run." $LASTEXITCODE
        }
    }

    Invoke-RunStage 'format' (Join-Path $ScriptDirectory 'format.ps1') @('--write')
    Invoke-RunStage 'lint' (Join-Path $ScriptDirectory 'lint.ps1') @()
    Invoke-RunStage 'test' 'cargo' @('test', '--locked', '--all-targets', '--features', 'test-support')
    Invoke-RunStage 'doctest' 'cargo' @('test', '--locked', '--doc', '--features', 'test-support')
    Invoke-RunStage 'build' 'cargo' @('build', '--locked')
    if (-not $noRun) {
        $runArguments = @('run', '--locked')
        if ($appArguments.Count -gt 0) {
            $runArguments += @('--') + $appArguments
        }
        Invoke-RunStage 'run' 'cargo' $runArguments
    }
    exit 0
}
catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    if (Get-Command Get-VerificationExitCode -ErrorAction SilentlyContinue) {
        exit (Get-VerificationExitCode $_)
    }
    exit 1
}
