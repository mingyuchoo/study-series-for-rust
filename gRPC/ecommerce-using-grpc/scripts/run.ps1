#!/usr/bin/env pwsh
# ecommerce-using-grpc: 포맷 + 빌드 + 테스트 + 서버/클라이언트 실행
[CmdletBinding()]
param(
    [Parameter(Position = 0, ValueFromRemainingArguments = $true)]
    [string[]] $Commands
)

$ErrorActionPreference = 'Stop'
$script:ProjectRoot = Split-Path -Parent $PSScriptRoot
$script:ServerProcess = $null
$script:WebProcess = $null
$script:StartedServer = $false

Set-Location $script:ProjectRoot

function Write-Log {
    param([string] $Message, [string] $Color = 'Cyan')
    Write-Host $Message -ForegroundColor $Color
}

function Write-Step {
    param([string] $Message)
    Write-Host "`n==> $Message" -ForegroundColor Cyan
}

function Invoke-CommandChecked {
    param([string] $Name, [string[]] $ArgumentList)
    & $Name @ArgumentList
    if ($LASTEXITCODE -ne 0) {
        throw "명령이 실패했습니다 (종료 코드 $LASTEXITCODE): $Name $($ArgumentList -join ' ')"
    }
}

function Require-Command {
    param([string] $Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "필수 명령을 찾을 수 없습니다: $Name"
    }
}

function Show-Usage {
    @'
Usage: .\scripts\run.ps1 [command...]

Commands:
  all            검사 + 포맷 + 린트 + 빌드 + 테스트 + gRPC 및 Web 서버 지속 실행 (기본값)
  web | run      gRPC 서버 백그라운드 기동 후 Web 대시보드(http://localhost:3000) 실행 (지속 실행)
  server         gRPC 서버 포그라운드 실행 ([::1]:50051)
  client         CLI 클라이언트 실행 (gRPC 서버가 이미 실행 중이어야 함)
  both           서버 백그라운드 기동 후 클라이언트 1회 실행 후 자동 종료
  ci | pipeline  검사 + 포맷 + 린트 + 빌드 + 테스트 + 클라이언트 1회 실행 후 자동 종료
  test           cargo test --workspace
  clippy         cargo clippy --workspace --all-targets -- -D warnings
  fmt | format   cargo fmt --all
  build          cargo build --profile dev
  release        cargo build --profile release
  check          필수 도구 확인 (cargo, rustc, protoc)
  help           이 도움말

Examples:
  .\scripts\run.ps1             # 전체 검사/빌드 후 gRPC 및 Web 대시보드 지속 실행
  .\scripts\run.ps1 web         # gRPC 및 Web 대시보드 실행 (http://localhost:3000)
  .\scripts\run.ps1 both        # 서버 기동 후 클라이언트 실행 후 종료
  .\scripts\run.ps1 ci          # CI용 파이프라인 검증 후 종료
  .\scripts\run.ps1 server      # gRPC 서버만 단독 실행
  .\scripts\run.ps1 client      # 클라이언트만 단독 실행
'@ | Write-Host
}

function Test-ServerReady {
    $tcp = $null
    try {
        $tcp = [System.Net.Sockets.TcpClient]::new()
        $async = $tcp.BeginConnect('::1', 50051, $null, $null)
        if (-not $async.AsyncWaitHandle.WaitOne(300)) { return $false }
        $tcp.EndConnect($async)
        return $true
    }
    catch { return $false }
    finally { if ($tcp) { $tcp.Dispose() } }
}

function Stop-ManagedProcess {
    param([System.Diagnostics.Process] $Process, [string] $Label)
    if ($null -eq $Process) { return }
    try {
        if (-not $Process.HasExited) {
            Write-Log "$Label 종료 중 (PID: $($Process.Id))..." 'Cyan'
            # cargo가 띄운 실제 서버/웹 프로세스까지 함께 정리
            & taskkill.exe /PID $Process.Id /T /F 2>$null | Out-Null
            if (-not $Process.HasExited) { $Process.Kill() }
            $Process.WaitForExit()
        }
    }
    catch { }
}

function Invoke-Cleanup {
    Write-Host ''
    Write-Log '서비스 종료 처리 중...' 'Cyan'
    Stop-ManagedProcess $script:WebProcess 'Web 대시보드'
    Stop-ManagedProcess $script:ServerProcess 'gRPC 서버'
    $script:WebProcess = $null
    $script:ServerProcess = $null
    Write-Log '관리 대상 서비스가 종료되었습니다.' 'Green'
}

function Start-GrpcServer {
    if (Test-ServerReady) {
        Write-Log '이미 [::1]:50051 에서 gRPC 서버가 실행 중입니다. 기존 서버에 연결합니다.' 'Yellow'
        return
    }

    Write-Log 'gRPC 서버 백그라운드 기동 중...' 'Cyan'
    $script:ServerProcess = Start-Process -FilePath 'cargo' -ArgumentList @('run', '-p', 'server') -WorkingDirectory $script:ProjectRoot -PassThru -NoNewWindow
    $script:StartedServer = $true
    Write-Log 'gRPC 서버 준비 대기 중...' 'Cyan'
    # 첫 실행에서는 cargo run이 의존성 및 서버를 컴파일할 수 있으므로
    # 포트 준비 시간은 빌드 제한(6초)과 분리해 충분히 기다립니다.
    $startupTimeoutSeconds = 180
    $deadline = [DateTime]::UtcNow.AddSeconds($startupTimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        $script:ServerProcess.Refresh()
        if ($script:ServerProcess.HasExited) {
            throw "gRPC 서버 프로세스가 준비 전에 종료되었습니다 (종료 코드: $($script:ServerProcess.ExitCode))."
        }
        if (Test-ServerReady) {
            Write-Log 'gRPC 서버 준비 완료 (주소: [::1]:50051)' 'Green'
            return
        }
        Start-Sleep -Milliseconds 300
    }
    throw "gRPC 서버 기동 대기 시간 초과 ($startupTimeoutSeconds초)"
}

function Invoke-Check {
    Write-Step '필수 도구 확인'
    Require-Command cargo
    Require-Command rustc
    if (-not (Get-Command protoc -ErrorAction SilentlyContinue)) {
        throw @'
필수 명령을 찾을 수 없습니다: protoc
프로토콜 코드 생성에 Protocol Buffers 컴파일러가 필요합니다.
Windows 설치 예: winget install Google.Protobuf
설치 후 새 PowerShell을 열거나 protoc.exe가 있는 디렉터리를 PATH에 추가한 뒤 다시 실행하세요.
'@
    }
    Write-Log "Rust: $(& rustc --version)" 'Green'
    Write-Log "Cargo: $(& cargo --version)" 'Green'
    Write-Log "protoc: $(& protoc --version)" 'Green'
}

function Invoke-Format {
    Write-Step '코드 포맷팅 (cargo fmt --all)'
    Require-Command cargo
    Invoke-CommandChecked cargo @('fmt', '--all')
    Write-Log '포맷팅 완료' 'Green'
}

function Invoke-Clippy {
    Write-Step 'Clippy (cargo clippy --workspace --all-targets)'
    Require-Command cargo
    Invoke-CommandChecked cargo @('clippy', '--workspace', '--all-targets', '--', '-D', 'warnings')
    Write-Log 'Clippy 완료' 'Green'
}

function Invoke-Build {
    Write-Step '빌드 (cargo build --profile dev)'
    Require-Command cargo
    Invoke-CommandChecked cargo @('build', '--profile', 'dev')
    Write-Log '빌드 완료' 'Green'
}

function Invoke-Release {
    Write-Step '릴리스 빌드 (cargo build --profile release)'
    Require-Command cargo
    Invoke-CommandChecked cargo @('build', '--profile', 'release')
    Write-Log '릴리스 빌드 완료' 'Green'
}

function Invoke-Tests {
    Write-Step '테스트 (cargo test --workspace)'
    Require-Command cargo
    Invoke-CommandChecked cargo @('test', '--workspace')
    Write-Log '테스트 완료' 'Green'
}

function Invoke-Server {
    Write-Step 'gRPC 서버 실행 (cargo run -p server)'
    Require-Command cargo
    Write-Log '리스닝 주소: [::1]:50051' 'Cyan'
    Invoke-CommandChecked cargo @('run', '-p', 'server')
}

function Invoke-Client {
    Write-Step '클라이언트 실행 (cargo run -p client)'
    Require-Command cargo
    Write-Log '연결 대상: http://[::1]:50051 (서버가 먼저 실행 중이어야 합니다)' 'Cyan'
    Invoke-CommandChecked cargo @('run', '-p', 'client')
}

function Invoke-Both {
    Write-Step '서버 및 클라이언트 실행 (단회 실행 테스트)'
    Require-Command cargo
    $startedHere = -not (Test-ServerReady)
    if (-not $startedHere) {
        Write-Log '이미 [::1]:50051 에서 서버가 실행 중입니다. 기존 서버에 연결합니다.' 'Yellow'
    } else {
        Start-GrpcServer
    }
    Write-Log '클라이언트 실행...' 'Cyan'
    Invoke-Client
    if ($startedHere) {
        Invoke-Cleanup
        Write-Log '서버 및 클라이언트 실행 테스트 완료' 'Green'
    }
}

function Invoke-Web {
    Write-Step 'gRPC 서버 및 Web 대시보드 실행 (지속 실행)'
    Require-Command cargo
    Start-GrpcServer
    Write-Log 'gRPC 클라이언트 통신 확인 및 샘플 데이터 등록...' 'Cyan'
    try { Invoke-Client } catch { Write-Log "클라이언트 초기화 실패 (계속 실행): $($_.Exception.Message)" 'Yellow' }

    Write-Host "`n======================================================================" -ForegroundColor Green
    Write-Host '🚀 e-Commerce gRPC 서비스 및 Web 대시보드가 정상 실행되었습니다!' -ForegroundColor Green
    Write-Host '   🌐 Web 대시보드:  http://localhost:3000' -ForegroundColor Cyan
    Write-Host '   ⚡ gRPC 서버:     http://[::1]:50051' -ForegroundColor Cyan
    Write-Host "`n   💡 브라우저에서 http://localhost:3000 에 접속하여 모니터링할 수 있습니다." -ForegroundColor Yellow
    Write-Host '   🛑 서비스를 종료하려면 Ctrl+C 를 누르세요.' -ForegroundColor Yellow
    Write-Host "======================================================================`n" -ForegroundColor Green
    try { Start-Process 'http://localhost:3000' -ErrorAction SilentlyContinue } catch { }

    try {
        $script:WebProcess = Start-Process -FilePath 'cargo' -ArgumentList @('run', '-p', 'web') -WorkingDirectory $script:ProjectRoot -PassThru -NoNewWindow
        $script:WebProcess.WaitForExit()
        if ($script:WebProcess.ExitCode -ne 0) { Write-Log "Web 프로세스 종료 코드: $($script:WebProcess.ExitCode)" 'Yellow' }
    }
    finally { Invoke-Cleanup }
}

function Invoke-CI {
    Invoke-Check
    Invoke-Format
    Invoke-Clippy
    Invoke-Build
    Invoke-Tests
    Invoke-Both
    Write-Host ''
    Write-Log '전체 파이프라인 검증 완료 (format + clippy + build + test + client)' 'Green'
}

function Invoke-All {
    Invoke-Check
    Invoke-Format
    Invoke-Clippy
    Invoke-Build
    Invoke-Tests
    Invoke-Web
}

try {
    if (-not $Commands -or $Commands.Count -eq 0) { Invoke-All; return }
    foreach ($command in $Commands) {
        switch -Regex ($command) {
            '^(help|-h|--help)$' { Show-Usage; break }
            '^check$' { Invoke-Check; break }
            '^(fmt|format)$' { Invoke-Format; break }
            '^clippy$' { Invoke-Clippy; break }
            '^build$' { Invoke-Build; break }
            '^release$' { Invoke-Release; break }
            '^test$' { Invoke-Tests; break }
            '^server$' { Invoke-Server; break }
            '^client$' { Invoke-Client; break }
            '^both$' { Invoke-Both; break }
            '^(web|run|serve)$' { Invoke-Web; break }
            '^(ci|pipeline|test-all)$' { Invoke-CI; break }
            '^all$' { Invoke-All; break }
            default { throw "알 수 없는 명령: $command`n`n$(Show-Usage)" }
        }
    }
}
catch {
    Write-Log "❌ $($_.Exception.Message)" 'Red'
    exit 1
}
finally {
    if ($script:ServerProcess -or $script:WebProcess) { Invoke-Cleanup }
}
