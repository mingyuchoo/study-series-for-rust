param([switch]$Write)
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
if ($Write) { & ./kotlin.bat run -m tooling -- format --write }
else { & ./kotlin.bat run -m tooling -- format }
exit $LASTEXITCODE
