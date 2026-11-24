#requires -Version 7.0
# Install repository-local CI tools; no administrator rights or global PATH changes.
[CmdletBinding()]
param([switch]$SkipDocker)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$tools = Join-Path $root 'target/local-ci/tools'
New-Item -ItemType Directory -Force $tools | Out-Null

function Install-VerifiedZip($Name, $Url, $Sha256, $Destination) {
    $archive = Join-Path $tools "$Name.zip"
    Invoke-WebRequest $Url -OutFile $archive
    if ((Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Sha256) {
        throw "Checksum mismatch for $Name"
    }
    Expand-Archive -LiteralPath $archive -DestinationPath $Destination -Force
    Remove-Item -LiteralPath $archive
}

if (-not [Environment]::Is64BitOperatingSystem -or $env:OS -ne 'Windows_NT') {
    throw 'This bootstrap supports Windows x64. See tools/dev/ci/README.md for Linux/macOS.'
}
foreach ($command in 'git', 'python', 'rustup', 'cargo', 'pwsh') {
    if (-not (Get-Command $command -ErrorAction SilentlyContinue)) { throw "$command is required on PATH" }
}
& python -c 'import sys; assert sys.version_info >= (3, 11), "Python 3.11+ required"'
if ($LASTEXITCODE) { throw 'Python 3.11+ is required' }
& rustup component add --toolchain stable rustfmt clippy
if ($LASTEXITCODE) { throw 'Installing Rust components failed' }

$act = Join-Path $tools 'act/act.exe'
if (-not (Test-Path $act)) {
    Install-VerifiedZip 'act' 'https://github.com/nektos/act/releases/download/v0.2.98/act_Windows_x86_64.zip' `
        'd0d868ee2b33bb710f4df82d69345ce3a10d2f9fc7bbee9522bf36a15779ada0' (Split-Path $act)
}
$node = Join-Path $tools 'node-v26.13.1-win-x64/node.exe'
if (-not (Test-Path $node)) {
    Install-VerifiedZip 'node' 'https://nodejs.org/dist/v26.13.1/node-v26.13.1-win-x64.zip' `
        'e2edda75bfb089654705cf65a13b58933bfd5f57148e8ebda916f58a2474f1f7' $tools
}
& $act --version
if ($LASTEXITCODE) { throw 'act is not usable' }
& $node --version
if ($LASTEXITCODE) { throw 'Node is not usable' }
if (-not $SkipDocker) {
    $dockerOs = & docker info --format '{{.OSType}}'
    if ($LASTEXITCODE -or $dockerOs -ne 'linux') { throw 'Start Docker Desktop with Linux containers, then rerun setup' }
    & docker build --tag swoopdl-ci:local (Join-Path $PSScriptRoot 'ci')
    if ($LASTEXITCODE) { throw 'Building local CI image failed' }
}
Write-Host 'Ready: pwsh -File tools/dev/ci.ps1'
