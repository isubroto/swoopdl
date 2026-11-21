# Swoopdl browser extension local setup
# Run by the optional post-install checkbox; no browser profile or policy is altered.
[CmdletBinding()]
param(
    [ValidateSet('Auto', 'Both', 'Chrome', 'Edge', 'Brave', 'Firefox')]
    [string]$Browser = 'Auto',
    [switch]$CheckOnly,
    [switch]$RequireFirefoxSigned,
    [switch]$RequireNativeHost,
    [switch]$NoPause,
    [string]$PackageRoot = ''
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$expectedChromeId = 'oogbclbbninliongijffeeggalppliok'
$expectedFirefoxId = 'integration@swoopdl.example'
$hostName = 'com.swoopdl.host'

function Say([string]$Text) { Write-Host ('[Swoopdl] ' + $Text) }
function Get-BrowserExe([string]$Name) {
    $relative = switch ($Name) {
        'Chrome' { 'Google\Chrome\Application\chrome.exe' }
        'Edge' { 'Microsoft\Edge\Application\msedge.exe' }
        'Brave' { 'BraveSoftware\Brave-Browser\Application\brave.exe' }
        'Firefox' { 'Mozilla Firefox\firefox.exe' }
    }
    foreach ($root in @($env:ProgramFiles, ${env:ProgramFiles(x86)}, $env:LOCALAPPDATA)) {
        if (-not $root) { continue }
        $p = Join-Path $root $relative
        if (Test-Path -LiteralPath $p -PathType Leaf) { return $p }
    }
    $command = Get-Command -Name ([IO.Path]::GetFileName($relative)) -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    return $null
}
function Get-ChromeId([string]$Base64Key) {
    if (-not $Base64Key) { throw 'Chromium manifest is missing the stable public key.' }
    $key = [Convert]::FromBase64String($Base64Key)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { $digest = $sha.ComputeHash($key) } finally { $sha.Dispose() }
    $alphabet = 'abcdefghijklmnop'
    $b = New-Object System.Text.StringBuilder
    foreach ($v in $digest[0..15]) {
        $high = [int]($v -shr 4)
        $low = [int]($v -band 15)
        [void]$b.Append($alphabet[$high])
        [void]$b.Append($alphabet[$low])
    }
    return $b.ToString()
}
function Read-XpiManifest([string]$XpiPath) {
    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    $archive = [IO.Compression.ZipFile]::OpenRead($XpiPath)
    try {
        $entry = $archive.GetEntry('manifest.json')
        if (-not $entry) { throw "Firefox XPI lacks manifest.json: $XpiPath" }
        $reader = New-Object IO.StreamReader($entry.Open())
        try { $manifest = $reader.ReadToEnd() | ConvertFrom-Json }
        finally { $reader.Dispose() }
        $signature = @($archive.Entries | Where-Object {
            $_.FullName -match '^META-INF/[^/]+\.(rsa|sig)$'
        }).Count -gt 0
        return @{ Manifest = $manifest; HasSignatureFiles = $signature }
    } finally { $archive.Dispose() }
}
function NativeHostStatus([string]$BrowserName, [string]$ExtensionId, [string]$ExpectedExecutable) {
    $branch = if ($BrowserName -eq 'Firefox') {
        'Software\Mozilla\NativeMessagingHosts'
    } else {
        'Software\Google\Chrome\NativeMessagingHosts'
    }
    foreach ($hive in @('HKEY_CURRENT_USER', 'HKEY_LOCAL_MACHINE')) {
        $regPath = "Registry::$hive\$branch\$hostName"
        if (-not (Test-Path -LiteralPath $regPath)) { continue }
        $manifestPath = (Get-Item -LiteralPath $regPath).GetValue('')
        if (-not $manifestPath -or -not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) { continue }
        $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
        $trusted = if ($BrowserName -eq 'Firefox') {
            @($manifest.allowed_extensions) -contains $ExtensionId
        } else {
            @($manifest.allowed_origins) -contains ('chrome-extension://' + $ExtensionId + '/')
        }
        $exeMatches = $manifest.path -and
            ([IO.Path]::GetFullPath([string]$manifest.path) -eq [IO.Path]::GetFullPath($ExpectedExecutable))
        if ($trusted -and $exeMatches -and $manifest.name -eq $hostName -and $manifest.type -eq 'stdio') {
            return "OK ($hive)"
        }
    }
    return 'NOT REGISTERED or manifest mismatch'
}

try {
    # Installed: {app}\extension\ ; source: {repo}\tools\extension\ with target\extension\.
    if ($PackageRoot) { $package = [IO.Path]::GetFullPath($PackageRoot) }
    elseif (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'chromium\manifest.json')) {
        $package = $PSScriptRoot
    } else {
        $repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
        $package = Join-Path $repo 'target\extension'
    }
    $chromeFolder = Join-Path $package 'chromium'
    $chromeManifestPath = Join-Path $chromeFolder 'manifest.json'
    $xpi = Join-Path $package 'sdl-firefox.xpi'
    if (-not (Test-Path -LiteralPath $chromeManifestPath -PathType Leaf)) { throw "Chromium package missing: $chromeManifestPath" }
    if (-not (Test-Path -LiteralPath $xpi -PathType Leaf)) { throw "Firefox package missing: $xpi" }
    $chromeManifest = Get-Content -LiteralPath $chromeManifestPath -Raw | ConvertFrom-Json
    if ((Get-ChromeId ([string]$chromeManifest.key)) -ne $expectedChromeId) {
        throw 'Chromium manifest public key gives a different ID than the trusted native host.'
    }
    if (-not (@($chromeManifest.permissions) -contains 'nativeMessaging')) {
        throw 'Chromium extension lacks nativeMessaging permission.'
    }
    $firefox = Read-XpiManifest $xpi
    if ($firefox.Manifest.browser_specific_settings.gecko.id -ne $expectedFirefoxId) {
        throw 'Firefox XPI ID does not match the native messaging allowed_extensions ID.'
    }
    if (-not (@($firefox.Manifest.permissions) -contains 'nativeMessaging')) {
        throw 'Firefox extension lacks nativeMessaging permission.'
    }
    if ($RequireFirefoxSigned -and -not $firefox.HasSignatureFiles) {
        throw 'Firefox XPI is UNSIGNED. Obtain Mozilla unlisted signing before a release install.'
    }
    Say 'Browser extension package manifests and IDs: PASS'

    # When installed, native host lives next to the application executable.
    $appDir = Split-Path -Parent $package
    $hostExe = Join-Path $appDir 'swoopdl-nmhost.exe'
    if (Test-Path -LiteralPath $hostExe -PathType Leaf) {
        foreach ($family in @('Chrome', 'Firefox')) {
            $id = if ($family -eq 'Firefox') { $expectedFirefoxId } else { $expectedChromeId }
            $status = NativeHostStatus $family $id $hostExe
            Say "$family native host: $status"
            if ($RequireNativeHost -and $status -notlike 'OK*') { throw "$family native host not registered." }
        }
    } elseif ($RequireNativeHost) {
        throw "Native host exe missing: $hostExe"
    } else {
        Say 'Native host registration: skipped (running from source/build output).'
    }
    if (-not $firefox.HasSignatureFiles) {
        Write-Warning 'Firefox XPI has no detected signature files. Permanent installation needs an AMO-signed unlisted XPI.'
    } else {
        Say 'Firefox XPI signature files present (Firefox must still verify authenticity).'
    }
    if ($CheckOnly) { Say 'Inspection finished; nothing launched or modified.'; exit 0 }

    if ($Browser -eq 'Auto') {
        foreach ($candidate in @('Chrome', 'Edge', 'Brave', 'Firefox')) {
            if (Get-BrowserExe $candidate) { $Browser = $candidate; break }
        }
        if ($Browser -eq 'Auto') { throw 'No supported browser found. See README.txt for manual local setup.' }
        Say "Recommended installed browser: $Browser"
    }

    # Portable archives register only when the user explicitly runs this setup helper.
    if (Test-Path -LiteralPath $hostExe -PathType Leaf) {
        & $hostExe --register
        if ($LASTEXITCODE) { throw 'Native messaging host registration failed.' }
    }

    if ($Browser -in @('Both', 'Chrome', 'Edge', 'Brave')) {
        $chromiumName = if ($Browser -eq 'Both') { 'Chrome' } else { $Browser }
        $chrome = Get-BrowserExe $chromiumName
        $extensionsPage = if ($chromiumName -eq 'Edge') { 'edge://extensions' } elseif ($chromiumName -eq 'Brave') { 'brave://extensions' } else { 'chrome://extensions' }
        Say "${chromiumName}: 1) Enable Developer mode. 2) Click Load unpacked. 3) Select:"
        Write-Host "  $chromeFolder"
        if ($chrome) {
            Start-Process -FilePath $chrome -ArgumentList $extensionsPage
            Start-Process -FilePath 'explorer.exe' -ArgumentList ('"' + $chromeFolder + '"')
        } else { Write-Warning "$chromiumName executable not found. Open $extensionsPage manually." }
        Say "Expected Chrome extension ID: $expectedChromeId"
    }
    if ($Browser -in @('Both', 'Firefox')) {
        $ffExe = Get-BrowserExe 'Firefox'
        if ($firefox.HasSignatureFiles) {
            Say 'FIREFOX: open the signed XPI in Firefox; approve the browser installation prompt:'
            Write-Host "  $xpi"
            if ($ffExe) {
                $url = ([uri]([IO.Path]::GetFullPath($xpi))).AbsoluteUri
                Start-Process -FilePath $ffExe -ArgumentList $url
            } else { Write-Warning 'Firefox executable not found.' }
        } else {
            Say 'FIREFOX DEVELOPMENT ONLY: about:debugging -> This Firefox -> Load Temporary Add-on -> select:'
            Write-Host "  $xpi"
            if ($ffExe) {
                Start-Process -FilePath $ffExe -ArgumentList 'about:debugging#/runtime/this-firefox'
                Start-Process -FilePath 'explorer.exe' -ArgumentList ('/select,"' + $xpi + '"')
            } else { Write-Warning 'Firefox executable not found.' }
            Say 'Temporary unsigned extensions do not survive normal browser restart.'
        }
    }
    Say 'Browser approval is required. No browser settings or enterprise policies were changed.'
    if (-not $NoPause) { [void](Read-Host 'Press Enter to close this setup assistant') }
    exit 0
} catch {
    [Console]::Error.WriteLine('[ERROR] ' + $_.Exception.Message)
    if (-not $NoPause) { [void](Read-Host 'Press Enter to close') }
    exit 1
}
