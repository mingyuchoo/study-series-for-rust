$ErrorActionPreference = 'Stop'

$powerShell = (Get-Process -Id $PID).Path
$output = & $powerShell -NoProfile -File (Join-Path $PSScriptRoot '../run.ps1') 2>&1
$exitCode = $LASTEXITCODE
$output | Write-Output

if ($exitCode -ne 0) {
    exit $exitCode
}
if ($output -match '\bwarning:') {
    throw 'The development pipeline must finish without warnings.'
}

Write-Output 'run.ps1 checks passed'
