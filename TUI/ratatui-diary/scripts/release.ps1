#Requires -Version 5.1
<#
.SYNOPSIS
    ratatui-diary를 릴리스 빌드하고 Windows 설치 파일(Setup.exe)을 만든다.

.DESCRIPTION
    검증 -> 릴리스 빌드 -> 스테이징 -> Inno Setup 컴파일 순으로 수행한다.
      1. Verify  : cargo fmt --check / clippy / test  (-SkipTest로 생략)
      2. Build   : cargo build --release -p ratatui-diary
      3. Stage   : dist/staging/ 에 exe와 문서를 모은다
      4. Package : dist/ratatui-diary.iss 를 생성하고 ISCC로 컴파일

    산출물: dist/ratatui-diary-<version>-<arch>-setup.exe

    생성되는 설치 파일의 특성:
      - 사용자 단위 설치 (%LOCALAPPDATA%\Programs\Ratatui Diary), 관리자 권한 불필요
      - 사용자 PATH 자동 등록/해제 (설치 마법사에서 선택 가능)
      - 시작 메뉴 바로가기 (설치 마법사에서 선택 가능)
      - 제어판 '앱 및 기능'에서 제거 가능

.PARAMETER SkipTest
    fmt/clippy/test 검증 단계를 건너뛴다.

.PARAMETER SkipBuild
    cargo 빌드를 건너뛰고 기존 릴리스 바이너리를 그대로 패키징한다.
    설치 마법사 설정만 수정하며 반복할 때 쓴다.

.PARAMETER Version
    설치 파일에 표기할 버전. 기본값은 Cargo.toml에서 읽어온다.

.PARAMETER Target
    크로스 빌드할 대상 트리플 (예: x86_64-pc-windows-msvc). 기본값은 호스트 트리플.

.PARAMETER OutDir
    산출물 디렉터리. 기본값은 프로젝트 루트의 dist/.

.PARAMETER InstallInno
    Inno Setup이 없을 때 winget으로 자동 설치한다.

.EXAMPLE
    ./scripts/release.ps1

.EXAMPLE
    ./scripts/release.ps1 -SkipTest

.EXAMPLE
    ./scripts/release.ps1 -Target x86_64-pc-windows-msvc -InstallInno
#>
[CmdletBinding()]
param(
    [switch] $SkipTest,
    [switch] $SkipBuild,
    [string] $Version,
    [string] $Target,
    [string] $OutDir,
    [switch] $InstallInno
)

$ErrorActionPreference = 'Stop'

# 설치 파일의 고유 식별자. 업그레이드/제거 시 동일 앱으로 인식되므로 절대 바꾸지 말 것.
$AppId        = '8F3A2C41-9D6E-4B77-A1C5-2E0B7D4F6A83'
$AppName      = 'Ratatui Diary'
$AppPublisher = 'mingyuchoo'
$AppUrl       = 'https://github.com/mingyuchoo/study-series-for-rust'
$BinName      = 'ratatui-diary'
$PackageName  = 'ratatui-diary'

# 스크립트 위치를 기준으로 프로젝트 루트(scripts/의 상위)로 이동한다.
$ProjectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $ProjectRoot

function Write-Step {
    param([Parameter(Mandatory)] [string] $Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Write-Done {
    param([Parameter(Mandatory)] [string] $Message)
    Write-Host "==> $Message" -ForegroundColor Green
}

function Invoke-Native {
    param(
        [Parameter(Mandatory)] [string]   $Name,
        [Parameter(Mandatory)] [string]   $FilePath,
        [Parameter(Mandatory)] [string[]] $Arguments
    )

    Write-Step "$Name : $FilePath $($Arguments -join ' ')"
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Name 단계 실패 (exit code: $LASTEXITCODE)"
    }
    Write-Done "$Name 완료"
}

# Inno Setup 컴파일러(ISCC.exe)를 PATH와 표준 설치 경로에서 찾는다.
function Find-Iscc {
    $cmd = Get-Command 'ISCC.exe' -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }

    $candidates = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles}\Inno Setup 6\ISCC.exe",
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
    )
    foreach ($path in $candidates) {
        if ($path -and (Test-Path -LiteralPath $path)) { return $path }
    }

    # 레지스트리의 설치 위치도 확인한다 (비표준 경로 대응).
    $keys = @(
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Inno Setup 6_is1',
        'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Inno Setup 6_is1',
        'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Inno Setup 6_is1'
    )
    foreach ($key in $keys) {
        try {
            $loc = (Get-ItemProperty -Path $key -Name 'InstallLocation' -ErrorAction Stop).InstallLocation
        }
        catch { continue }
        if ($loc) {
            $exe = Join-Path $loc 'ISCC.exe'
            if (Test-Path -LiteralPath $exe) { return $exe }
        }
    }

    return $null
}

# rustc 트리플을 Inno Setup의 ArchitecturesAllowed 값과 파일명 라벨로 변환한다.
function Get-ArchInfo {
    param([Parameter(Mandatory)] [string] $Triple)

    switch -Regex ($Triple) {
        '^aarch64-' { return @{ Label = 'arm64'; Inno = 'arm64' } }
        '^x86_64-'  { return @{ Label = 'x64';   Inno = 'x64compatible' } }
        '^i686-'    { return @{ Label = 'x86';   Inno = 'x86compatible' } }
        default     { throw "지원하지 않는 대상 트리플입니다: $Triple" }
    }
}

try {
    # ------------------------------------------------------------------
    # 0) 사전 확인: 툴체인과 메타데이터
    # ------------------------------------------------------------------
    foreach ($tool in @('cargo', 'rustc')) {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            throw "$tool 를 찾을 수 없습니다. Rust 툴체인을 설치하세요: https://rustup.rs"
        }
    }

    # 호스트 트리플은 rustc -vV의 'host:' 줄에서 가져온다.
    $hostTriple = (& rustc -vV | Select-String -Pattern '^host:\s*(.+)$').Matches[0].Groups[1].Value.Trim()
    if (-not $Target) { $Target = $hostTriple }
    $arch = Get-ArchInfo -Triple $Target

    # 버전은 cargo metadata에서 읽는다 (workspace.package 상속까지 해석됨).
    if (-not $Version) {
        $metaJson = & cargo metadata --no-deps --format-version 1
        if ($LASTEXITCODE -ne 0) { throw "cargo metadata 실행 실패 (exit code: $LASTEXITCODE)" }
        $meta = $metaJson | ConvertFrom-Json
        $pkg = $meta.packages | Where-Object { $_.name -eq $PackageName }
        if (-not $pkg) { throw "워크스페이스에서 '$PackageName' 패키지를 찾을 수 없습니다." }
        $Version = $pkg.version
        $targetDir = $meta.target_directory
    }
    else {
        $metaJson = & cargo metadata --no-deps --format-version 1
        if ($LASTEXITCODE -ne 0) { throw "cargo metadata 실행 실패 (exit code: $LASTEXITCODE)" }
        $targetDir = ($metaJson | ConvertFrom-Json).target_directory
    }

    # Inno Setup의 VersionInfoVersion은 숫자 형식만 허용하므로 접미사를 떼어낸다.
    $numericVersion = ([regex]::Match($Version, '^\d+(\.\d+){0,3}')).Value
    if (-not $numericVersion) { $numericVersion = '0.0.0' }

    if (-not $OutDir) { $OutDir = Join-Path $ProjectRoot 'dist' }
    $OutDir     = [System.IO.Path]::GetFullPath($OutDir)
    $stagingDir = Join-Path $OutDir 'staging'
    $issPath    = Join-Path $OutDir "$PackageName.iss"
    $outBase    = "$PackageName-$Version-$($arch.Label)-setup"
    $setupPath  = Join-Path $OutDir "$outBase.exe"

    Write-Host ""
    Write-Host "  패키지 : $PackageName $Version" -ForegroundColor White
    Write-Host "  대상   : $Target ($($arch.Label))" -ForegroundColor White
    Write-Host "  산출물 : $setupPath" -ForegroundColor White

    # ------------------------------------------------------------------
    # 1) 검증: 포맷 / 린트 / 테스트
    # ------------------------------------------------------------------
    if (-not $SkipTest) {
        # --check는 파일을 수정하지 않고 포맷 위반만 보고한다 (릴리스 트리 불변 유지).
        Write-Step 'Verify : cargo fmt --all -- --check'
        & cargo fmt --all -- --check
        if ($LASTEXITCODE -ne 0) {
            throw "포맷이 맞지 않습니다. 'cargo fmt --all' 실행 후 다시 시도하세요."
        }
        Write-Done 'Format 확인 완료'

        Invoke-Native -Name 'Clippy' -FilePath 'cargo' -Arguments @('clippy', '--workspace', '--all-targets')
        Invoke-Native -Name 'Test'   -FilePath 'cargo' -Arguments @('test', '--workspace')
    }
    else {
        Write-Host ""
        Write-Host "==> Verify 단계 건너뜀 (-SkipTest)" -ForegroundColor Yellow
    }

    # ------------------------------------------------------------------
    # 2) 릴리스 빌드
    # ------------------------------------------------------------------
    # --target을 명시하면 산출 경로에 트리플 디렉터리가 한 단계 더 생긴다.
    $useExplicitTarget = ($Target -ne $hostTriple) -or $PSBoundParameters.ContainsKey('Target')
    if ($useExplicitTarget) {
        $binPath = Join-Path $targetDir "$Target\release\$BinName.exe"
    }
    else {
        $binPath = Join-Path $targetDir "release\$BinName.exe"
    }

    if (-not $SkipBuild) {
        $buildArgs = @('build', '--release', '-p', $PackageName)
        if ($useExplicitTarget) { $buildArgs += @('--target', $Target) }
        Invoke-Native -Name 'Build' -FilePath 'cargo' -Arguments $buildArgs
    }
    else {
        Write-Host ""
        Write-Host "==> Build 단계 건너뜀 (-SkipBuild)" -ForegroundColor Yellow
    }

    if (-not (Test-Path -LiteralPath $binPath)) {
        throw "릴리스 바이너리를 찾을 수 없습니다: $binPath"
    }

    # ------------------------------------------------------------------
    # 3) 스테이징: 설치 파일에 담을 내용을 한 곳에 모은다
    # ------------------------------------------------------------------
    Write-Step "Stage : $stagingDir"
    if (Test-Path -LiteralPath $stagingDir) {
        Remove-Item -LiteralPath $stagingDir -Recurse -Force
    }
    New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null

    Copy-Item -LiteralPath $binPath -Destination $stagingDir -Force
    foreach ($doc in @('README.md', 'LICENSE', 'LICENSE.md', 'LICENSE.txt')) {
        $docPath = Join-Path $ProjectRoot $doc
        if (Test-Path -LiteralPath $docPath) {
            Copy-Item -LiteralPath $docPath -Destination $stagingDir -Force
        }
    }
    # 실제로 스테이징된 파일만 [Files]에 넣는다. 와일드카드를 쓰면 매칭되는 파일이
    # 없을 때 ISCC가 컴파일 에러를 내므로, 목록을 여기서 확정한다.
    $stagedFiles = @(Get-ChildItem -LiteralPath $stagingDir -File)
    $filesSection = ($stagedFiles | ForEach-Object {
        Write-Host ("    + {0} ({1:N0} bytes)" -f $_.Name, $_.Length) -ForegroundColor DarkGray
        'Source: "{0}"; DestDir: "{{app}}"; Flags: ignoreversion' -f $_.Name
    }) -join "`r`n"
    Write-Done 'Stage 완료'

    # ------------------------------------------------------------------
    # 4) .iss 생성
    # ------------------------------------------------------------------
    # PATH 등록/해제는 [Code]에서 직접 처리한다. Inno Setup은 [Registry]로 추가한
    # PATH 항목을 제거 시 자동으로 되돌려주지 않기 때문이다.
    $iss = @"
; 이 파일은 scripts/release.ps1이 자동 생성합니다. 직접 수정하지 마세요.
; 설치 마법사 설정을 바꾸려면 release.ps1의 `$iss 템플릿을 수정하세요.

[Setup]
AppId={{$AppId}
AppName=$AppName
AppVersion=$Version
VersionInfoVersion=$numericVersion
AppPublisher=$AppPublisher
AppPublisherURL=$AppUrl
AppSupportURL=$AppUrl
DefaultDirName={autopf}\$AppName
DefaultGroupName=$AppName
UninstallDisplayName=$AppName $Version
UninstallDisplayIcon={app}\$BinName.exe
OutputDir=$OutDir
OutputBaseFilename=$outBase
SourceDir=$stagingDir
; 사용자 단위 설치: {autopf}가 %LOCALAPPDATA%\Programs로 해석되어 UAC가 필요 없다.
PrivilegesRequired=lowest
ArchitecturesAllowed=$($arch.Inno)
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
AllowNoIcons=yes
DisableProgramGroupPage=yes
; PATH를 건드리므로 Windows에 환경 변수 변경을 알린다.
ChangesEnvironment=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "addtopath";  Description: "PATH에 추가 (터미널에서 $BinName 명령으로 실행)"; GroupDescription: "추가 작업:"
Name: "startmenu";  Description: "시작 메뉴에 바로가기 만들기";                    GroupDescription: "추가 작업:"

[Files]
$filesSection

[Icons]
Name: "{autoprograms}\$AppName"; Filename: "{app}\$BinName.exe"; Tasks: startmenu

[Code]
const
  EnvironmentKey = 'Environment';

{ 사용자 PATH 끝에 경로를 덧붙인다. 이미 있으면 아무것도 하지 않는다. }
procedure EnvAddPath(InstlPath: string);
var
  Paths: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Paths) then
    Paths := '';

  if Pos(';' + Uppercase(InstlPath) + ';', ';' + Uppercase(Paths) + ';') > 0 then
    exit;

  if Paths = '' then
    Paths := InstlPath
  else if Paths[Length(Paths)] = ';' then
    Paths := Paths + InstlPath
  else
    Paths := Paths + ';' + InstlPath;

  RegWriteExpandStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Paths);
end;

{ 사용자 PATH에서 경로를 제거한다. 앞뒤 구분자까지 정확히 한 개만 지운다. }
procedure EnvRemovePath(InstlPath: string);
var
  Paths: string;
  P: Integer;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Paths) then
    exit;

  P := Pos(';' + Uppercase(InstlPath) + ';', ';' + Uppercase(Paths) + ';');
  if P = 0 then
    exit;

  { P는 앞에 ';'를 덧댄 문자열 기준이므로, 맨 앞 항목일 때만 인덱스가 다르다. }
  if P = 1 then
    Delete(Paths, 1, Length(InstlPath) + 1)
  else
    Delete(Paths, P - 1, Length(InstlPath) + 1);

  RegWriteExpandStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Paths);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    EnvAddPath(ExpandConstant('{app}'));
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    EnvRemovePath(ExpandConstant('{app}'));
end;
"@

    Write-Step "Generate : $issPath"
    # Inno Setup 6은 BOM이 있어야 UTF-8 .iss의 비ASCII 문자를 올바로 읽는다.
    [System.IO.File]::WriteAllText($issPath, $iss, (New-Object System.Text.UTF8Encoding($true)))
    Write-Done 'Generate 완료'

    # ------------------------------------------------------------------
    # 5) Inno Setup 컴파일러 확보
    # ------------------------------------------------------------------
    $iscc = Find-Iscc
    if (-not $iscc -and $InstallInno) {
        if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
            throw 'winget을 찾을 수 없어 Inno Setup을 자동 설치할 수 없습니다.'
        }
        Invoke-Native -Name 'Install Inno Setup' -FilePath 'winget' -Arguments @(
            'install', '--id', 'JRSoftware.InnoSetup', '--exact', '--silent',
            '--accept-package-agreements', '--accept-source-agreements'
        )
        $iscc = Find-Iscc
    }
    if (-not $iscc) {
        # 여기까지 왔다면 스테이징과 .iss 생성은 끝난 상태다. Inno Setup 설치 후
        # -SkipTest -SkipBuild 로 다시 실행하면 패키징만 이어서 할 수 있다.
        throw @'
Inno Setup 컴파일러(ISCC.exe)를 찾을 수 없습니다. 아래 중 하나를 선택하세요.
  - 이 스크립트에 -InstallInno 스위치를 붙여 다시 실행
  - 직접 설치: winget install --id JRSoftware.InnoSetup --exact
  - 수동 설치: https://jrsoftware.org/isdl.php  (Inno Setup 6.3 이상)
'@
    }
    Write-Host ""
    Write-Host "  ISCC   : $iscc" -ForegroundColor White

    # ------------------------------------------------------------------
    # 6) 설치 파일 컴파일
    # ------------------------------------------------------------------
    Invoke-Native -Name 'Package' -FilePath $iscc -Arguments @($issPath)

    if (-not (Test-Path -LiteralPath $setupPath)) {
        throw "설치 파일이 생성되지 않았습니다: $setupPath"
    }
    $setupSize = (Get-Item -LiteralPath $setupPath).Length

    Write-Host ""
    Write-Host "설치 파일 생성 완료" -ForegroundColor Green
    Write-Host "  $setupPath" -ForegroundColor White
    Write-Host ("  {0:N2} MB" -f ($setupSize / 1MB)) -ForegroundColor White
    Write-Host ""
    Write-Host "설치하려면 위 파일을 실행하세요. PATH를 선택했다면 새 터미널에서 '$BinName' 명령을 쓸 수 있습니다." -ForegroundColor DarkGray
}
catch {
    Write-Host ""
    Write-Host "!! $($_.Exception.Message)" -ForegroundColor Red
    Pop-Location
    exit 1
}

Pop-Location
