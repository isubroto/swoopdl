# Builds the browser extension for each browser family from extension/ (one background.js):
#   target/extension/chromium/  - manifest.json (Chrome, Edge, Brave, Opera, Vivaldi; load unpacked)
#   target/extension/firefox/   - manifest.firefox.json as manifest.json
#   target/extension/safari/    - manifest.safari.json as manifest.json (a folder only: tools/safari
#                                 builds it into the app as Safari's extension, macOS)
#   target/extension/sdl-chromium.zip - the chromium folder, zipped (a release download: unzip it
#                                       and load the folder unpacked)
#   target/extension/sdl-firefox.xpi  - the firefox folder, zipped (Firefox needs it signed by AMO
#                                       to install for good; about:debugging loads it unsigned)
#   target/extension/sdl-chrome-store.zip - what is uploaded to the Chrome Web Store (see below)
# Both manifests get the app's version (Cargo.toml), so an extension and its app always match.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$src = Join-Path $root "extension"
# Joined part by part: .NET (the zips below) takes a backslash literally on Linux and macOS.
$out = Join-Path (Join-Path $root "target") "extension"
$cargo = Get-Content (Join-Path $root "Cargo.toml") -Raw
$version = [regex]::Match($cargo, '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
if (-not $version) { throw "no version in Cargo.toml" }
# The stores take one to four dot-separated numbers only (no "-rc.1").
if ($version -notmatch '^\d+(\.\d+){0,3}$') { throw "version ${version}: browsers take only numbers like 1.2.3" }
foreach ($kind in "chromium", "firefox", "safari") {
  $dir = Join-Path $out $kind
  if (Test-Path $dir) { Remove-Item -LiteralPath $dir -Recurse -Force }
  New-Item -ItemType Directory -Force $dir | Out-Null
  # The background part; the download button over videos and the Alt-click watcher (content
  # scripts in every page); the toolbar button's popup.
  foreach ($f in "background.js", "video.js", "page.js", "popup.html", "popup.css", "popup.js", "help.html") {
    Copy-Item (Join-Path $src $f) $dir
  }
  Copy-Item (Join-Path $src "icons") (Join-Path $dir "icons") -Recurse
  # Its texts in the browser's language (tools/extension/locales.py).
  Copy-Item (Join-Path $src "_locales") (Join-Path $dir "_locales") -Recurse
  # Norwegian: Chrome takes only "no", Firefox looks for "nb" (Bokmål) as its users' browsers say.
  if ($kind -eq "firefox" -and (Test-Path (Join-Path $dir "_locales/no"))) {
    Copy-Item (Join-Path $dir "_locales/no") (Join-Path $dir "_locales/nb") -Recurse
  }
  $manifest = switch ($kind) { "firefox" { "manifest.firefox.json" } "safari" { "manifest.safari.json" } default { "manifest.json" } }
  $json = Get-Content (Join-Path $src $manifest) -Raw
  # The first "version" only (the manifest's own).
  $json = ([regex]'("version"\s*:\s*)"[^"]*"').Replace($json, "`${1}`"$version`"", 1)
  [IO.File]::WriteAllText((Join-Path $dir "manifest.json"), $json, (New-Object Text.UTF8Encoding $false))
}
# The Chrome Web Store's copy of the chromium folder. The store signs it with its own key (it
# refuses a manifest that carries "key"; the native host knows both IDs).
$store = Join-Path $out "chrome-store"
if (Test-Path $store) { Remove-Item -LiteralPath $store -Recurse -Force }
Copy-Item (Join-Path $out "chromium") $store -Recurse
$manifest = Get-Content (Join-Path $store "manifest.json") -Raw | ConvertFrom-Json
$manifest.PSObject.Properties.Remove("key")
$json = $manifest | ConvertTo-Json -Depth 10
[IO.File]::WriteAllText((Join-Path $store "manifest.json"), $json, (New-Object Text.UTF8Encoding $false))
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
# One offline bundle is shared by every platform's installer and extracted archive.
$bundle = Join-Path $out "bundle"
if (Test-Path -LiteralPath $bundle) { Remove-Item -LiteralPath $bundle -Recurse -Force }
New-Item -ItemType Directory -Force $bundle | Out-Null
Copy-Item -LiteralPath (Join-Path $out "chromium") -Destination $bundle -Recurse
# Firefox's XPI is copied after the browser packages are zipped below.
foreach ($helper in "install-local.ps1", "install-local.sh", "Set up browser extension.command") {
  Copy-Item -LiteralPath (Join-Path $PSScriptRoot $helper) -Destination $bundle
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "LOCAL-SETUP.txt") -Destination (Join-Path $bundle "README.txt")
# The zips have manifest.json at their root. Entries are named with "/" as zip requires
# (Windows PowerShell's CreateFromDirectory writes "\", which Firefox rejects).
foreach ($pair in @(@("chromium", "sdl-chromium.zip"), @("firefox", "sdl-firefox.xpi"), @("chrome-store", "sdl-chrome-store.zip"))) {
  $dir = (Resolve-Path (Join-Path $out $pair[0])).Path
  $zip = Join-Path $out $pair[1]
  if (Test-Path $zip) { Remove-Item -LiteralPath $zip -Force }
  $archive = [System.IO.Compression.ZipFile]::Open($zip, [System.IO.Compression.ZipArchiveMode]::Create)
  try {
    foreach ($f in Get-ChildItem -LiteralPath $dir -Recurse -File | Sort-Object FullName) {
      $name = $f.FullName.Substring($dir.Length).TrimStart('\', '/').Replace('\', '/')
      [void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $f.FullName, $name)
    }
  } finally { $archive.Dispose() }
}
"built $out (chromium, firefox, safari, chrome-store, sdl-chromium.zip, sdl-firefox.xpi, sdl-chrome-store.zip; version $version)"
Copy-Item -LiteralPath (Join-Path $out "sdl-firefox.xpi") -Destination $bundle
$localZip = Join-Path $out "sdl-local.zip"
if (Test-Path -LiteralPath $localZip) { Remove-Item -LiteralPath $localZip -Force }
$archive = [System.IO.Compression.ZipFile]::Open($localZip, [System.IO.Compression.ZipArchiveMode]::Create)
try {
  foreach ($file in Get-ChildItem -LiteralPath $bundle -Recurse -File | Sort-Object FullName) {
    $entry = $file.FullName.Substring($bundle.Length).TrimStart('\', '/').Replace('\', '/')
    [void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $file.FullName, $entry)
  }
} finally { $archive.Dispose() }
"built $localZip (offline installer bundle)"
