#requires -Version 7.4
# Tool gate only: independent review + AC evidence are required for final PASS.
try {
    . (Join-Path $PSScriptRoot '_common.ps1')
    # Git and child PowerShell failures are checked explicitly below.
    $PSNativeCommandUseErrorActionPreference = $false
    $git = Get-Command git -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $git) { Stop-Verification 'FAIL: git required' }
    $GitExecutable = $git.Source
    $PowerShellExecutable = (Join-Path $PSHOME $(if ($IsWindows) { 'pwsh.exe' } else { 'pwsh' }))

    function Invoke-VerificationGit {
        param([string[]]$Arguments)
        $output = & $GitExecutable @Arguments 2>&1
        $status = $LASTEXITCODE
        if ($status -ne 0) { Stop-Verification "FAIL: git $($Arguments -join ' ') (exit $status)" }
        return $output
    }

    $revision = [string](Invoke-VerificationGit @('rev-parse', '--verify', 'HEAD'))
    $gitRoot = [IO.Path]::GetFullPath([string](Invoke-VerificationGit @('rev-parse', '--show-toplevel')))
    $comparison = if ($IsWindows) { [StringComparison]::OrdinalIgnoreCase } else { [StringComparison]::Ordinal }
    if (-not [string]::Equals($gitRoot, $RepositoryRoot, $comparison)) {
        Stop-Verification 'FAIL: place template at Git repository root'
    }
    if (@(Invoke-VerificationGit @('status', '--porcelain', '--untracked-files=all')).Count -ne 0) {
        Stop-Verification 'FAIL: commit changes before verification (clean tree required)'
    }
    & $GitExecutable check-ignore -q .artifacts/verification/probe
    if ($LASTEXITCODE -ne 0) { Stop-Verification 'FAIL: add /.artifacts/ to repository .gitignore' }

    $runName = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ') + '-' + [guid]::NewGuid().ToString('N')
    $runDirectory = Join-Path $RepositoryRoot ".artifacts/verification/$runName"
    $null = New-Item -ItemType Directory -Path $runDirectory
    $resultFile = Join-Path $runDirectory 'result.env'
    Set-Content -LiteralPath $resultFile -Value 'TOOL_GATE=FAIL' -Encoding utf8
    Set-Content -LiteralPath (Join-Path $runDirectory 'revision.txt') -Value $revision -Encoding utf8
    $summaryFile = Join-Path $runDirectory 'summary.tsv'
    Set-Content -LiteralPath $summaryFile -Value "step`tstatus`texit_code" -Encoding utf8
    Write-Output "Evidence: $runDirectory"

    function Test-VerificationRevision {
        $head = [string](Invoke-VerificationGit @('rev-parse', 'HEAD'))
        $changes = @(Invoke-VerificationGit @('status', '--porcelain', '--untracked-files=all'))
        return ($head -eq $revision -and $changes.Count -eq 0)
    }

    function Invoke-VerificationStep {
        param([string]$Step, [string]$Script, [string[]]$ScriptArguments = @())
        if (-not (Test-VerificationRevision)) {
            Add-Content -LiteralPath $summaryFile -Value "$Step`tFAIL_REVISION_CHANGED`t1" -Encoding utf8
            Stop-Verification 'FAIL: revision or working tree changed' 1
        }
        $log = Join-Path $runDirectory "$Step.log"
        try {
            & $PowerShellExecutable -NoProfile -File (Join-Path $ScriptDirectory $Script) @ScriptArguments 2>&1 |
                Tee-Object -FilePath $log
            $status = $LASTEXITCODE
        }
        catch {
            Add-Content -LiteralPath $summaryFile -Value "$Step`tFAIL`t1" -Encoding utf8
            Stop-Verification "FAIL: $Step output/logging failed: $($_.Exception.Message)" 1
        }
        if ($status -ne 0) {
            Add-Content -LiteralPath $summaryFile -Value "$Step`tFAIL`t$status" -Encoding utf8
            Stop-Verification "FAIL: $Step (exit $status). Remaining steps not run." $status
        }
        if (-not (Test-VerificationRevision)) {
            Add-Content -LiteralPath $summaryFile -Value "$Step`tFAIL_REVISION_CHANGED`t1" -Encoding utf8
            Stop-Verification "FAIL: $Step modified revision or working tree" 1
        }
        Add-Content -LiteralPath $summaryFile -Value "$Step`tPASS`t0" -Encoding utf8
    }

    Invoke-VerificationStep 'format' 'format.ps1' @('--check')
    Invoke-VerificationStep 'lint' 'lint.ps1'
    Invoke-VerificationStep 'unit' 'unit-test.ps1'
    Invoke-VerificationStep 'integration' 'integration-test.ps1'
    Invoke-VerificationStep 'e2e' 'e2e-test.ps1'
    Set-Content -LiteralPath $resultFile -Value 'TOOL_GATE=PASS' -Encoding utf8
    Write-Output "TOOL PASS: $revision. Final gate still requires AC evidence and independent review."
    exit 0
}
catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit (Get-VerificationExitCode $_)
}
