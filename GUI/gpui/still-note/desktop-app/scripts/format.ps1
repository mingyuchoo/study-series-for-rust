#requires -Version 7.4
try {
    . (Join-Path $PSScriptRoot '_common.ps1')
    $mode = if ($args.Count -eq 0) { '--check' } else { $args[0] }
    switch ($mode) {
        '--check' { Invoke-VerificationCommand 'FORMAT_CHECK_CMD' }
        '--write' { Invoke-VerificationCommand 'FORMAT_WRITE_CMD' }
        default { Stop-Verification 'Usage: pwsh -File scripts/format.ps1 [--check|--write]' }
    }
    exit 0
}
catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit (Get-VerificationExitCode $_)
}
