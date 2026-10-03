#requires -Version 7.4
try {
    . (Join-Path $PSScriptRoot '_common.ps1')
    Invoke-VerificationCommand 'LINT_CMD'
    exit 0
}
catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit (Get-VerificationExitCode $_)
}
