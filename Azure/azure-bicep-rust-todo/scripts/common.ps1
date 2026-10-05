$ProjectRoot = Split-Path -Parent $PSScriptRoot
$FrontendDirectory = Join-Path $ProjectRoot 'src/frontend'
$BackendDirectory = Join-Path $ProjectRoot 'src/backend'
$DockerComposeFile = Join-Path $ProjectRoot 'src/docker/docker-compose.yml'

function Invoke-InDirectory {
    param([string]$Path, [scriptblock]$Action)
    Push-Location -LiteralPath $Path
    try { & $Action } finally { Pop-Location }
}
function Invoke-Pnpm {
    $commandArguments = @($args)
    $packageManager = (Get-Content -LiteralPath (Join-Path $FrontendDirectory 'package.json') -Raw | ConvertFrom-Json).packageManager
    & npx --yes $packageManager @commandArguments
    if ($LASTEXITCODE -ne 0) { throw "pnpm failed with exit code $LASTEXITCODE" }
}
function Invoke-Cargo {
    $commandArguments = @($args)
    & cargo @commandArguments
    if ($LASTEXITCODE -ne 0) { throw "cargo failed with exit code $LASTEXITCODE" }
}
function Build-Frontend {
    Write-Host 'Building SolidJS frontend...' -ForegroundColor Yellow
    Invoke-InDirectory $FrontendDirectory {
        Invoke-Pnpm install --frozen-lockfile
        Invoke-Pnpm run build:backend
    }
}
