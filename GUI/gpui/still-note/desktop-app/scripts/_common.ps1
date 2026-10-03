#requires -Version 7.4
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$ScriptDirectory = $PSScriptRoot
$RepositoryRoot = [IO.Path]::GetFullPath((Join-Path $ScriptDirectory '..'))
Set-Location -LiteralPath $RepositoryRoot

function Get-VerificationExitCode {
    param([System.Management.Automation.ErrorRecord]$Failure)
    # PowerShell's native-command error retains the original process exit status.
    $exception = $Failure.Exception
    while ($null -ne $exception) {
        if ($exception -is [System.Management.Automation.NativeCommandExitException]) {
            return $exception.ExitCode
        }
        if ($exception.Data.Contains('VerificationExitCode')) {
            return [int]$exception.Data['VerificationExitCode']
        }
        $exception = $exception.InnerException
    }
    return 1
}

function Stop-Verification {
    param([string]$Message, [int]$ExitCode = 2)
    $failure = [InvalidOperationException]::new($Message)
    $failure.Data['VerificationExitCode'] = $ExitCode
    throw $failure
}

function Invoke-VerificationCommand {
    param([string]$Key)
    $config = Join-Path $RepositoryRoot '.agents/verification.ps1'
    if (-not (Test-Path -LiteralPath $config -PathType Leaf)) {
        Stop-Verification "FAIL: missing $config"
    }
    # Configuration is reviewed executable PowerShell code, not a dotenv file.
    $FORMAT_WRITE_CMD = ''
    $FORMAT_CHECK_CMD = ''
    $LINT_CMD = ''
    $UNIT_TEST_CMD = ''
    $INTEGRATION_TEST_CMD = ''
    $E2E_TEST_CMD = ''
    . $config
    $command = Get-Variable -Name $Key -ValueOnly
    if ($command -isnot [string] -or [string]::IsNullOrWhiteSpace($command)) {
        Stop-Verification "FAIL: configure $Key in .agents/verification.ps1"
    }
    Write-Output "Running $Key"
    & ([scriptblock]::Create($command))
}
