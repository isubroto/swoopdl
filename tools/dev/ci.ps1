#requires -Version 7.0
# Run pre-push checks against the working tree. Each failure returns a nonzero exit code.
[CmdletBinding()]
param(
    [ValidateSet('all', 'rust', 'extension', 'website', 'pictures', 'translations')]
    [string]$Check = 'all',
    [string]$Base = 'origin/main',
    [switch]$Linux,
    [ValidateSet('rust', 'extension', 'website', 'pictures')]
    [string]$Job = 'rust',
    [ValidateRange(1, 128)]
    [int]$BuildJobs = 1,
    [switch]$DryRun
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$tools = Join-Path $root 'target/local-ci/tools'
$localNode = Join-Path $tools 'node-v26.13.1-win-x64'
$oldPath = $env:PATH
$oldUtf8 = $env:PYTHONUTF8
$oldBuildJobs = $env:CARGO_BUILD_JOBS
$oldTestDebug = $env:CARGO_PROFILE_TEST_DEBUG
$log = Join-Path $root ("target/local-ci/logs/{0}-{1}.log" -f (Get-Date -Format 'yyyyMMdd-HHmmss-fff'), $PID)
$script:failures = @()

function Invoke-Check([string]$Name, [string]$Program, [string[]]$Arguments) {
    Write-Host "== $Name"
    Write-Host "$Program $($Arguments -join ' ')"
    if ($DryRun) { return }
    if (-not (Get-Command $Program -ErrorAction SilentlyContinue)) {
        throw "$Program is missing. Run pwsh -File tools/dev/setup-ci.ps1 first."
    }
    & $Program @Arguments 2>&1 | Tee-Object -FilePath $log -Append | Out-Host
    if ($LASTEXITCODE -ne 0) { $script:failures += "$Name (exit $LASTEXITCODE)" }
}

Push-Location $root
try {
    $env:PYTHONUTF8 = '1'
    $env:CARGO_BUILD_JOBS = "$BuildJobs"
    $env:CARGO_PROFILE_TEST_DEBUG = 'line-tables-only'
    if (Test-Path $localNode) { $env:PATH = "$localNode$([IO.Path]::PathSeparator)$env:PATH" }
    if (-not $DryRun) { New-Item -ItemType Directory -Force (Split-Path $log) | Out-Null }
    if ($Linux) {
        $act = Join-Path $tools 'act/act.exe'
        if (-not (Test-Path $act)) { $act = 'act' }
        $workflow = 'ci.yml'
        $jobId = $Job
        $event = 'push'
        $extra = @()
        switch ($Job) {
            'rust' { $extra = @('--matrix', 'os:ubuntu-latest') }
            'website' { $workflow = 'pages.yml'; $jobId = 'build'; $event = 'workflow_dispatch' }
            'pictures' { $workflow = 'help-pictures.yml'; $jobId = 'check' }
        }
        $arguments = @($event, '-W', ".github/workflows/$workflow", '-j', $jobId,
            '-P', 'ubuntu-latest=swoopdl-ci:local', '--pull=false',
            '--container-architecture', 'linux/amd64',
            '--container-daemon-socket', '-',
            '--env', "CARGO_BUILD_JOBS=$BuildJobs",
            '--artifact-server-path', (Join-Path $root 'target/local-ci/artifacts')) + $extra
        Invoke-Check "Linux workflow: $Job" $act $arguments
    } else {
        $checks = if ($Check -eq 'all') { @('rust', 'extension', 'website', 'pictures', 'translations') } else { @($Check) }
        foreach ($item in $checks) {
            switch ($item) {
                'rust' {
                    Invoke-Check 'Format' cargo @('fmt', '--all', '--check')
                    # --check compares without rewriting the developer's catalog.
                    Invoke-Check 'English catalog' python @('tools/i18n/extract.py', '--check')
                    Invoke-Check 'Clippy' cargo @('clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings')
                    if ($IsWindows) {
                        # Match CI's serial Windows harness without changing workspace features.
                        Invoke-Check 'Tests' cargo @('test', '--workspace', '--locked', '--', '--test-threads=1')
                    } else {
                        Invoke-Check 'Tests' cargo @('test', '--workspace', '--locked')
                    }
                }
                'extension' {
                    if (-not $DryRun) {
                        $major = & node -p 'process.versions.node.split(".")[0]'
                        if ($LASTEXITCODE -or $major -ne '26') { throw 'Node 26 required. Run tools/dev/setup-ci.ps1.' }
                    }
                    Invoke-Check 'Extension logic' node @('tools/extension/test.mjs')
                    Invoke-Check 'Extension packaging' pwsh @('-NoProfile', '-File', 'tools/extension/pack.ps1')
                    Invoke-Check 'Extension packages' node @('tools/extension/check-pack.mjs')
                    Invoke-Check 'Offline bundle' python @('tools/extension/test-bundle.py')
                }
                'website' {
                    Invoke-Check 'Website tests' python @('-m', 'unittest', 'discover', '-s', 'tools/website', '-p', 'test_build.py', '-v')
                    Invoke-Check 'Website build' python @('tools/website/build.py')
                }
                'pictures' { Invoke-Check 'Help pictures' python @('tools/shots/make.py', 'check') }
                'translations' {
                    if (-not $DryRun) {
                        & git rev-parse --verify "$Base^{commit}" | Out-Null
                        if ($LASTEXITCODE) { throw "Base '$Base' is missing. Fetch it or pass -Base <ref>." }
                    }
                    Invoke-Check 'Reviewed translations' python @('tools/i18n/reviewed_guard.py', '--base', $Base)
                }
            }
        }
    }
    if ($script:failures.Count) { throw "Failed: $($script:failures -join ', '). Log: $log" }
    if ($DryRun) { Write-Host 'Dry run only; no checks executed.' }
    else { Write-Host "Selected checks passed. Log: $log" }
} finally {
    $env:PATH = $oldPath
    $env:PYTHONUTF8 = $oldUtf8
    $env:CARGO_BUILD_JOBS = $oldBuildJobs
    $env:CARGO_PROFILE_TEST_DEBUG = $oldTestDebug
    Pop-Location
}
