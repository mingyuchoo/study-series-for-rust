#requires -Version 7.4
param(
    [Parameter(Mandatory)]
    [ValidateSet('format-write', 'format-check', 'lint', 'unit', 'integration', 'e2e')]
    [string] $Mode
)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$Pwsh = (Get-Command pwsh -CommandType Application | Select-Object -First 1).Source
$Git = (Get-Command git -CommandType Application | Select-Object -First 1).Source
$Keys = @('FORMAT_WRITE_CMD', 'FORMAT_CHECK_CMD', 'LINT_CMD', 'UNIT_TEST_CMD', 'INTEGRATION_TEST_CMD', 'E2E_TEST_CMD')
$Results = [Collections.Generic.List[object]]::new()
$RunId = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ') + '-' + [guid]::NewGuid().ToString('N')
$Evidence = Join-Path $Root ".artifacts/powershell/tests/$Mode-$RunId"

function Assert-True([bool] $Condition, [string] $Message) {
    if (-not $Condition) { throw "ASSERT: $Message" }
}

function Write-Utf8([string] $Path, [string] $Content) {
    [IO.File]::WriteAllText($Path, $Content, [Text.UTF8Encoding]::new($false))
}

function Quote-String([string] $Text) {
    return "'" + $Text.Replace("'", "''") + "'"
}

function Invoke-Process([string] $Executable, [string[]] $Arguments, [string] $Cwd, [switch] $WithoutPath) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $Executable
    $info.WorkingDirectory = $Cwd
    $info.UseShellExecute = $false
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    if ($WithoutPath) { $info.Environment['PATH'] = '' }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    [void] $process.Start()
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $result = [pscustomobject]@{
        ExitCode = $process.ExitCode
        Stdout = $stdoutTask.GetAwaiter().GetResult()
        Stderr = $stderrTask.GetAwaiter().GetResult()
    }
    $process.Dispose()
    return $result
}

function Invoke-Git([string] $Cwd, [string[]] $Arguments) {
    $result = Invoke-Process $Git $Arguments $Cwd
    Assert-True ($result.ExitCode -eq 0) "Fixture Git failed: $($Arguments -join ' '): $($result.Stderr)"
    return $result.Stdout.Trim()
}

function Set-Config([string] $Fixture, [hashtable] $Overrides = @{}) {
    $lines = foreach ($key in $Keys) {
        $command = "Add-Content -LiteralPath '.artifacts/dispatch.txt' -Value ('${key}::' + (Get-Location).Path)"
        if ($Overrides.ContainsKey($key)) { $command = $Overrides[$key] }
        '$' + $key + ' = ' + (Quote-String $command)
    }
    Write-Utf8 (Join-Path $Fixture '.agents/verification.ps1') (($lines -join "`n") + "`n")
}

function New-Fixture([switch] $WithoutGit, [switch] $WithoutIgnore, [switch] $BlankConfig) {
    $fixture = Join-Path ([IO.Path]::GetTempPath()) ('ps-scripts-' + [guid]::NewGuid().ToString('N'))
    [void] (New-Item -ItemType Directory -Path $fixture)
    [void] (New-Item -ItemType Directory -Path (Join-Path $fixture '.agents'))
    [void] (New-Item -ItemType Directory -Path (Join-Path $fixture '.artifacts'))
    Copy-Item -LiteralPath (Join-Path $Root 'scripts') -Destination $fixture -Recurse
    Copy-Item -LiteralPath (Join-Path $Root '.agents/verification.ps1') -Destination (Join-Path $fixture '.agents/verification.ps1')
    if (-not $BlankConfig) { Set-Config $fixture }
    Write-Utf8 (Join-Path $fixture 'tracked.txt') "original`n"
    if (-not $WithoutIgnore) { Write-Utf8 (Join-Path $fixture '.gitignore') "/.artifacts/`n" }
    if (-not $WithoutGit) {
        [void] (Invoke-Git $fixture @('init', '--quiet'))
        [void] (Invoke-Git $fixture @('config', 'user.name', 'PowerShell verifier fixture'))
        [void] (Invoke-Git $fixture @('config', 'user.email', 'verifier@example.invalid'))
        Commit-Fixture $fixture
    }
    return $fixture
}

function Commit-Fixture([string] $Fixture) {
    [void] (Invoke-Git $Fixture @('add', '--all'))
    [void] (Invoke-Git $Fixture @('commit', '--quiet', '--allow-empty', '-m', 'Fixture checkpoint'))
}

function Invoke-Script([string] $Fixture, [string] $Script, [string[]] $Arguments = @(), [switch] $WithoutPath) {
    $result = Invoke-Process $Pwsh (@('-NoLogo', '-NoProfile', '-File', (Join-Path $Fixture "scripts/$Script.ps1")) + $Arguments) ([IO.Path]::GetTempPath()) -WithoutPath:$WithoutPath
    $logId = [guid]::NewGuid().ToString('N')
    Write-Utf8 (Join-Path $Evidence "$Script-$logId.stdout.log") $result.Stdout
    Write-Utf8 (Join-Path $Evidence "$Script-$logId.stderr.log") $result.Stderr
    return $result
}

function Get-Run([string] $Fixture) {
    $runs = @(Get-ChildItem -LiteralPath (Join-Path $Fixture '.artifacts/verification') -Directory)
    Assert-True ($runs.Count -eq 1) "Expected one evidence run, observed $($runs.Count)"
    return $runs[0].FullName
}

function Assert-FailEvidence([string] $Fixture, [string] $Step) {
    $run = Get-Run $Fixture
    Assert-True ((Get-Content -LiteralPath (Join-Path $run 'result.env') -Raw).Trim() -eq 'TOOL_GATE=FAIL') 'Failure retains default FAIL gate'
    $summary = Get-Content -LiteralPath (Join-Path $run 'summary.tsv') -Raw
    Assert-True ($summary -match "(?m)^$Step\tFAIL") "Summary contains failed $Step step: $summary"
    Assert-True ($summary -notmatch '(?m)^e2e\tPASS') 'Failure never reports later e2e PASS'
}

function Invoke-Case([string] $Name, [string] $Ac, [scriptblock] $Body) {
    try {
        & $Body
        $Results.Add([pscustomobject]@{ Name = $Name; AC = $Ac; Status = 'PASS'; Error = '' })
        Write-Output "PASS [$Ac] $Name"
    } catch {
        $Results.Add([pscustomobject]@{ Name = $Name; AC = $Ac; Status = 'FAIL'; Error = $_.ToString() })
        Write-Output "FAIL [$Ac] $Name : $_"
    }
}

function Get-SourceFiles {
    @((Get-ChildItem -LiteralPath (Join-Path $Root 'scripts') -Filter '*.ps1').FullName) +
        @((Get-ChildItem -LiteralPath $PSScriptRoot -Filter '*.ps1' -Recurse).FullName) +
        @(Join-Path $Root '.agents/verification.ps1')
}

if ($Mode -in @('format-write', 'format-check')) {
    $dirty = @()
    foreach ($path in Get-SourceFiles) {
        $before = [IO.File]::ReadAllText($path)
        $after = (($before -replace "`r`n", "`n" -replace "`r", "`n") -replace '(?m)[ \t]+$', '').TrimEnd("`n") + "`n"
        if ($before -cne $after) {
            $dirty += $path
            if ($Mode -eq 'format-write') { Write-Utf8 $path $after }
        }
    }
    if ($Mode -eq 'format-check' -and $dirty.Count -gt 0) {
        Write-Error "Formatting differences: $($dirty -join ', ')"
        exit 1
    }
    Write-Output "FORMAT PASS: $Mode; files=$(@(Get-SourceFiles).Count); changed=$($dirty.Count)"
    exit 0
}

if ($Mode -eq 'lint') {
    foreach ($path in Get-SourceFiles) {
        $tokens = $null
        $parseErrors = $null
        [void] [Management.Automation.Language.Parser]::ParseFile($path, [ref] $tokens, [ref] $parseErrors)
        Assert-True ($parseErrors.Count -eq 0) "PowerShell parse errors in $path : $parseErrors"
        $content = [IO.File]::ReadAllText($path)
        Assert-True (-not $content.Contains([char] 0)) "NUL character in source: $path"
        Assert-True (-not ($content -match '(?m)`[ \t]+$')) "Invalid continuation with whitespace: $path"
    }
    Write-Output "LINT PASS: parsed $(@(Get-SourceFiles).Count) PowerShell files; checked NUL and invalid continuation"
    exit 0
}

[void] (New-Item -ItemType Directory -Path $Evidence -Force)

if ($Mode -eq 'unit') {
    Invoke-Case 'eight equivalents parse and Bash scripts retain reviewed baseline hashes' 'AC-01' {
        $expected = @{
            '_common.sh' = 'D5D55780E44967971A5DA0B3ED597925EFA0EFE03E8D2355B35A981E739A36FF'
            'e2e-test.sh' = 'CF3C4D17D63A85F447F593DCF16292F8F05E1D261869A50DE4DC6034F74F8F61'
            'format.sh' = '06409C04EBA83A7A226DB77379BF3C0191DE93B561AEE5639EE99187E559FEBA'
            'integration-test.sh' = 'F735787D63E4B1DD767F77A7EF5B5C238F60718F9602779BEFE06F6A4D82CC3E'
            'lint.sh' = 'A36AAAC347F25A35FA4AEE73B9967C7D2CC2213202FC374A77DDFFC80D36B6B7'
            'unit-test.sh' = '3F19E2712BD67805E6C4DCA1F7FC7994845E47E5127611F520E5F57580C119CF'
            # Intentional nested Cargo project support; behavior is independently
            # exercised by tests/run-scripts/test_run_scripts.py.
            'verify.sh' = 'BB8EAFDB977EF0D9DE74EBA32B6E2947E552B663CE956E94B9E6B1AFA589C385'
            'run.sh' = '70745D20DD58B162F1914080A70FFEC957ABF6F89827568455AE7EB7175E154E'
        }
        $shellFiles = @(Get-ChildItem -LiteralPath (Join-Path $Root 'scripts') -Filter '*.sh')
        Assert-True ($shellFiles.Count -eq 8) 'All eight reviewed Bash scripts remain'
        Assert-True ((@($shellFiles.Name | Sort-Object) -join ',') -eq (@($expected.Keys | Sort-Object) -join ',')) 'Exact reviewed script set remains'
        foreach ($shell in $shellFiles) {
            Assert-True ((Get-FileHash -LiteralPath $shell.FullName).Hash -eq $expected[$shell.Name]) "Reviewed Bash baseline unchanged: $($shell.Name)"
            $equivalent = [IO.Path]::ChangeExtension($shell.FullName, '.ps1')
            Assert-True (Test-Path -LiteralPath $equivalent) "Equivalent exists: $equivalent"
            $tokens = $null
            $parseErrors = $null
            $ast = [Management.Automation.Language.Parser]::ParseFile($equivalent, [ref] $tokens, [ref] $parseErrors)
            Assert-True ($parseErrors.Count -eq 0) "Equivalent parses: $equivalent"
            Assert-True ($ast.ScriptRequirements.RequiredPSVersion -ge [version] '7.4') "Equivalent enforces stable native failure support from PowerShell 7.4: $equivalent"
        }
    }
    Invoke-Case 'common module is dot-sourceable' 'AC-01' {
        $command = '. ' + (Quote-String (Join-Path $Root 'scripts/_common.ps1')) + '; exit 0'
        $result = Invoke-Process $Pwsh @('-NoProfile', '-Command', $command) ([IO.Path]::GetTempPath())
        Assert-True ($result.ExitCode -eq 0) "Common module dot-sources: $($result.Stderr)"
    }
    Invoke-Case 'all six blank command keys fail closed' 'AC-03' {
        $fixture = New-Fixture
        $blank = @{}
        foreach ($key in $Keys) { $blank[$key] = '' }
        Set-Config $fixture $blank
        foreach ($case in @(@('format', @('--write')), @('format', @('--check')), @('lint', @()), @('unit-test', @()), @('integration-test', @()), @('e2e-test', @()))) {
            $result = Invoke-Script $fixture $case[0] $case[1]
            Assert-True ($result.ExitCode -eq 2) "Blank command fails closed for $($case[0])"
        }
    }
}

if ($Mode -eq 'integration') {
    Invoke-Case 'all wrappers dispatch proper keys at root from another cwd' 'AC-02' {
        $fixture = New-Fixture
        $cases = @(
            @('format', @(), 'FORMAT_CHECK_CMD'), @('format', @('--check'), 'FORMAT_CHECK_CMD'),
            @('format', @('--write'), 'FORMAT_WRITE_CMD'), @('lint', @(), 'LINT_CMD'),
            @('unit-test', @(), 'UNIT_TEST_CMD'), @('integration-test', @(), 'INTEGRATION_TEST_CMD'), @('e2e-test', @(), 'E2E_TEST_CMD')
        )
        foreach ($case in $cases) {
            $result = Invoke-Script $fixture $case[0] $case[1]
            Assert-True ($result.ExitCode -eq 0) "Wrapper $($case[0]) passes: $($result.Stderr)"
        }
        $lines = @(Get-Content -LiteralPath (Join-Path $fixture '.artifacts/dispatch.txt'))
        Assert-True ($lines.Count -eq 7) 'Exactly seven configured commands ran'
        for ($i = 0; $i -lt $cases.Count; $i++) { Assert-True ($lines[$i] -eq ($cases[$i][2] + '::' + $fixture)) "Correct key and root for dispatch $i : $($lines[$i])" }
    }
    Invoke-Case 'invalid format mode exits 2 without invoking command' 'AC-02' {
        $fixture = New-Fixture
        $result = Invoke-Script $fixture 'format' @('--wrong')
        Assert-True ($result.ExitCode -eq 2) 'Invalid mode exit is 2'
        Assert-True (-not (Test-Path -LiteralPath (Join-Path $fixture '.artifacts/dispatch.txt'))) 'Invalid mode runs no commands'
    }
    Invoke-Case 'missing and whitespace-only config fail exit 2' 'AC-03' {
        $fixture = New-Fixture
        Remove-Item -LiteralPath (Join-Path $fixture '.agents/verification.ps1')
        $result = Invoke-Script $fixture 'lint'
        Assert-True ($result.ExitCode -eq 2) 'Missing config exit is 2'
        Set-Config $fixture @{ LINT_CMD = " `t " }
        $result = Invoke-Script $fixture 'lint'
        Assert-True ($result.ExitCode -eq 2) 'Whitespace config exit is 2'
    }
    Invoke-Case 'throw and native failure stop trailing success' 'AC-03' {
        foreach ($native in @($false, $true)) {
            $fixture = New-Fixture
            $prefix = if ($native) { '& ' + (Quote-String $Pwsh) + ' -NoProfile -Command "exit 23"' } else { 'throw "deliberate command error"' }
            Set-Config $fixture @{ LINT_CMD = $prefix + '; Set-Content -LiteralPath ".artifacts/later.txt" -Value "bad"' }
            $result = Invoke-Script $fixture 'lint'
            Assert-True ($result.ExitCode -ne 0) 'Failing command exits nonzero'
            if ($native) { Assert-True ($result.ExitCode -eq 23) "Native exit 23 preserved, actual $($result.ExitCode)" }
            Assert-True (-not (Test-Path -LiteralPath (Join-Path $fixture '.artifacts/later.txt'))) 'Trailing success cannot mask failure'
        }
    }
    Invoke-Case 'trusted config errors stop before command' 'AC-03' {
        foreach ($native in @($false, $true)) {
            $fixture = New-Fixture
            $config = Join-Path $fixture '.agents/verification.ps1'
            $prefix = if ($native) { '& ' + (Quote-String $Pwsh) + ' -NoProfile -Command "exit 27"' } else { 'throw "deliberate config error"' }
            Write-Utf8 $config ($prefix + "`n" + [IO.File]::ReadAllText($config))
            $result = Invoke-Script $fixture 'lint'
            Assert-True ($result.ExitCode -ne 0) 'Config error exits nonzero'
            if ($native) { Assert-True ($result.ExitCode -eq 27) 'Config native exit 27 preserved' }
            Assert-True (-not (Test-Path -LiteralPath (Join-Path $fixture '.artifacts/dispatch.txt'))) 'Config error does not dispatch command'
        }
    }
    Invoke-Case 'verify rejects absent git and absent checkpoint' 'AC-05' {
        $fixture = New-Fixture
        $result = Invoke-Script $fixture 'verify' -WithoutPath
        Assert-True ($result.ExitCode -eq 2) 'Absent Git exits 2'
        $fixture = New-Fixture -WithoutGit
        $result = Invoke-Script $fixture 'verify'
        Assert-True ($result.ExitCode -eq 2) 'Absent repository/checkpoint exits 2'
        [void] (Invoke-Git $fixture @('init', '--quiet'))
        $result = Invoke-Script $fixture 'verify'
        Assert-True ($result.ExitCode -eq 2) 'Unborn HEAD exits 2'
    }
    Invoke-Case 'verify rejects dirty tree and nonignored artifacts' 'AC-05' {
        $fixture = New-Fixture
        Write-Utf8 (Join-Path $fixture 'tracked.txt') 'changed'
        $result = Invoke-Script $fixture 'verify'
        Assert-True ($result.ExitCode -eq 2) 'Dirty tracked tree exits 2'
        $fixture = New-Fixture
        Write-Utf8 (Join-Path $fixture 'untracked.txt') 'changed'
        $result = Invoke-Script $fixture 'verify'
        Assert-True ($result.ExitCode -eq 2) 'Untracked file exits 2'
        $fixture = New-Fixture -WithoutIgnore
        $result = Invoke-Script $fixture 'verify'
        Assert-True ($result.ExitCode -eq 2) 'Unignored artifacts exits 2'
    }
    Invoke-Case 'verify rejects nested template inside outer git root' 'AC-05' {
        $fixture = New-Fixture
        $nested = Join-Path $fixture 'nested'
        [void] (New-Item -ItemType Directory -Path $nested)
        Copy-Item -LiteralPath (Join-Path $fixture 'scripts') -Destination $nested -Recurse
        Copy-Item -LiteralPath (Join-Path $fixture '.agents') -Destination $nested -Recurse
        Commit-Fixture $fixture
        $result = Invoke-Script $nested 'verify'
        Assert-True ($result.ExitCode -eq 2) 'Wrong root exits 2'
    }
}

if ($Mode -eq 'e2e') {
    Invoke-Case 'clean fixture runs ordered steps with unique revision evidence and both streams' 'AC-04' {
        $fixture = New-Fixture
        $normal = "Add-Content -LiteralPath '.artifacts/dispatch.txt' -Value ('LINT_CMD::' + (Get-Location).Path); [Console]::Out.WriteLine('stdout-marker'); [Console]::Error.WriteLine('stderr-marker')"
        Set-Config $fixture @{ LINT_CMD = $normal }
        Commit-Fixture $fixture
        $revision = Invoke-Git $fixture @('rev-parse', 'HEAD')
        $result = Invoke-Script $fixture 'verify'
        Assert-True ($result.ExitCode -eq 0) "Successful verify exits 0: $($result.Stderr)"
        $run = Get-Run $fixture
        Assert-True ((Get-Content -LiteralPath (Join-Path $run 'revision.txt') -Raw).Trim() -eq $revision) 'Evidence revision equals fixture commit'
        $summary = @(Get-Content -LiteralPath (Join-Path $run 'summary.tsv'))
        $expected = @("step`tstatus`texit_code", "format`tPASS`t0", "lint`tPASS`t0", "unit`tPASS`t0", "integration`tPASS`t0", "e2e`tPASS`t0")
        Assert-True (($summary -join "`n") -ceq ($expected -join "`n")) 'Five steps have exact order and statuses'
        Assert-True ((Get-Content -LiteralPath (Join-Path $run 'result.env') -Raw).Trim() -eq 'TOOL_GATE=PASS') 'Successful evidence gate PASS'
        foreach ($step in @('format', 'lint', 'unit', 'integration', 'e2e')) { Assert-True (Test-Path -LiteralPath (Join-Path $run "$step.log")) "Step log exists: $step" }
        $lintLog = Get-Content -LiteralPath (Join-Path $run 'lint.log') -Raw
        Assert-True ($lintLog.Contains('stdout-marker') -and $lintLog.Contains('stderr-marker')) 'Log retains stdout and stderr'
        $dispatch = @(Get-Content -LiteralPath (Join-Path $fixture '.artifacts/dispatch.txt'))
        Assert-True ($dispatch.Count -eq 5) 'Exactly five verification commands executed'
        $keysInOrder = @('FORMAT_CHECK_CMD', 'LINT_CMD', 'UNIT_TEST_CMD', 'INTEGRATION_TEST_CMD', 'E2E_TEST_CMD')
        for ($i = 0; $i -lt 5; $i++) { Assert-True ($dispatch[$i] -eq ($keysInOrder[$i] + '::' + $fixture)) "Command execution ordering at $i" }
        $result = Invoke-Script $fixture 'verify'
        Assert-True ($result.ExitCode -eq 0) 'Second verify passes'
        $runs = @(Get-ChildItem -LiteralPath (Join-Path $fixture '.artifacts/verification') -Directory)
        Assert-True ($runs.Count -eq 2) 'Two invocations produce unique evidence directories'
        Assert-True ((Invoke-Git $fixture @('status', '--porcelain', '--untracked-files=all')) -eq '') 'Fixture remains clean after evidence'
    }
    Invoke-Case 'blank and missing config retain default FAIL evidence and stop' 'AC-03,AC-05' {
        foreach ($missing in @($false, $true)) {
            $fixture = New-Fixture
            if ($missing) { Remove-Item -LiteralPath (Join-Path $fixture '.agents/verification.ps1') } else { Set-Config $fixture @{ FORMAT_CHECK_CMD = '' } }
            Commit-Fixture $fixture
            $result = Invoke-Script $fixture 'verify'
            Assert-True ($result.ExitCode -eq 2) 'Invalid config verify exits 2'
            Assert-FailEvidence $fixture 'format'
            Assert-True (-not (Test-Path -LiteralPath (Join-Path $fixture '.artifacts/dispatch.txt'))) 'Config failure runs no commands'
        }
    }
    Invoke-Case 'native failing step preserves exit status and prevents later stages' 'AC-03,AC-05' {
        $fixture = New-Fixture
        Set-Config $fixture @{ LINT_CMD = '& ' + (Quote-String $Pwsh) + ' -NoProfile -Command "exit 23"; Set-Content -LiteralPath ".artifacts/later.txt" -Value "bad"' }
        Commit-Fixture $fixture
        $result = Invoke-Script $fixture 'verify'
        Assert-True ($result.ExitCode -eq 23) "Verify preserves failed native exit 23: $($result.ExitCode)"
        Assert-FailEvidence $fixture 'lint'
        Assert-True (-not (Test-Path -LiteralPath (Join-Path $fixture '.artifacts/later.txt'))) 'No trailing success after native failure'
        Assert-True (@(Get-Content -LiteralPath (Join-Path $fixture '.artifacts/dispatch.txt')).Count -eq 1) 'Only format runs before lint failure'
    }
    Invoke-Case 'changed HEAD or tracked/untracked tree fails immediately after step' 'AC-05' {
        foreach ($mutation in @('head', 'tracked', 'untracked')) {
            $fixture = New-Fixture
            $command = switch ($mutation) {
                'head' { '& ' + (Quote-String $Git) + ' commit --quiet --allow-empty -m "changed checkpoint"' }
                'tracked' { 'Set-Content -LiteralPath "tracked.txt" -Value "changed"' }
                'untracked' { 'Set-Content -LiteralPath "new.txt" -Value "changed"' }
            }
            Set-Config $fixture @{ FORMAT_CHECK_CMD = $command }
            Commit-Fixture $fixture
            $result = Invoke-Script $fixture 'verify'
            Assert-True ($result.ExitCode -ne 0) "Mutation $mutation fails verification"
            Assert-FailEvidence $fixture 'format'
            Assert-True (-not (Test-Path -LiteralPath (Join-Path $fixture '.artifacts/dispatch.txt'))) "Mutation $mutation prevents later stages"
        }
    }
    Invoke-Case 'log path failure cannot produce PASS or continue to unit' 'AC-05' {
        $fixture = New-Fixture
        $command = '$run = Get-ChildItem -LiteralPath ".artifacts/verification" -Directory | Select-Object -First 1; New-Item -ItemType Directory -Path (Join-Path $run.FullName "lint.log") | Out-Null'
        Set-Config $fixture @{ FORMAT_CHECK_CMD = $command }
        Commit-Fixture $fixture
        $result = Invoke-Script $fixture 'verify'
        Assert-True ($result.ExitCode -ne 0) 'Logging failure exits nonzero'
        Assert-FailEvidence $fixture 'lint'
        $dispatchPath = Join-Path $fixture '.artifacts/dispatch.txt'
        if (Test-Path -LiteralPath $dispatchPath) {
            $dispatch = Get-Content -LiteralPath $dispatchPath -Raw
            Assert-True (-not $dispatch.Contains('UNIT_TEST_CMD')) 'Logging failure prevents unit and later stages'
        }
    }
}

$Results | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $Evidence 'results.json') -Encoding utf8NoBOM
$failed = @($Results | Where-Object Status -eq 'FAIL').Count
Write-Output "TEST SUMMARY: category=$Mode; executed=$($Results.Count); passed=$($Results.Count - $failed); failed=$failed; skipped=0; evidence=$Evidence"
if ($Results.Count -eq 0 -or $failed -ne 0) { exit 1 }
exit 0
