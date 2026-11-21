#!/bin/sh
# Set up the extensions shipped with Swoopdl; never download an extension or change browser policies.
set -eu
package=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
family=auto
check_only=false
pause=true
while [ "$#" -gt 0 ]; do
  case "$1" in
    --browser) [ "$#" -ge 2 ] || { echo 'Missing browser family' >&2; exit 1; }; family=$2; shift ;;
    --check-only) check_only=true ;;
    --no-pause) pause=false ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
  shift
done
case "$family" in auto|chromium|firefox) ;; *) echo 'Use auto, chromium, or firefox.' >&2; exit 1 ;; esac
for file in chromium/manifest.json sdl-firefox.xpi README.txt; do
  [ -f "$package/$file" ] || { echo "Missing bundled file: $file" >&2; exit 1; }
done
if "$check_only"; then
  echo 'Local extension files present; nothing launched or registered.'
  exit 0
fi

browser=
page=
platform=$(uname -s)
if [ "$family" != firefox ]; then
  if [ "$platform" = Darwin ]; then
    for app in 'Google Chrome' 'Microsoft Edge' 'Brave Browser' Chromium; do
      for apps in /Applications "$HOME/Applications"; do
        if [ -d "$apps/$app.app" ]; then browser="$apps/$app.app"; break 2; fi
      done
    done
  else
    for command in google-chrome google-chrome-stable microsoft-edge brave-browser chromium chromium-browser; do
      if command -v "$command" >/dev/null 2>&1; then browser=$(command -v "$command"); break; fi
    done
  fi
  if [ -n "$browser" ]; then page=chrome://extensions; family=chromium; fi
fi
if [ -z "$browser" ] && [ "$family" != chromium ]; then
  family=firefox
  if [ "$platform" = Darwin ]; then
    for apps in /Applications "$HOME/Applications"; do
      if [ -d "$apps/Firefox.app" ]; then browser="$apps/Firefox.app"; break; fi
    done
  elif command -v firefox >/dev/null 2>&1; then
    browser=$(command -v firefox)
  fi
  page='about:debugging#/runtime/this-firefox'
fi
if [ -z "$browser" ]; then
  echo "No supported browser found. See $package/README.txt for manual setup." >&2
  exit 1
fi

# Installed Linux, extracted archives, and macOS app bundles use different host locations.
host=
for candidate in "$package/../swoopdl-nmhost" "$package/../../../bin/swoopdl-nmhost" "$package/../../MacOS/swoopdl-nmhost"; do
  if [ -x "$candidate" ]; then host=$candidate; break; fi
done
if [ -z "$host" ]; then echo 'Bundled native host not found.' >&2; exit 1; fi
case "$package" in
  /tmp/.mount_*|/private/tmp/.mount_*)
    echo 'Extract the AppImage to a permanent folder before setup, or install the .deb/.rpm package.' >&2
    exit 1 ;;
  /Volumes/*)
    echo 'Copy Swoopdl.app to Applications before running browser setup.' >&2
    exit 1 ;;
esac
"$host" --register

echo "Setting up $browser from bundled files. Browser approval is required."
if [ "$family" = chromium ]; then
  echo 'Enable Developer mode, choose Load unpacked, and select:'
  printf '  %s\n' "$package/chromium"
else
  # Signature presence is a hint only: Firefox verifies the signature itself.
  if command -v unzip >/dev/null 2>&1 && unzip -Z1 "$package/sdl-firefox.xpi" | grep -Eq '^META-INF/[^/]+\.(rsa|sig)$'; then
    page="$package/sdl-firefox.xpi"
    echo 'Install the bundled signed XPI from file in Firefox and approve its permissions:'
  else
    echo 'Unsigned Firefox package: choose Load Temporary Add-on and select:'
    echo 'Temporary add-ons disappear when Firefox restarts. Permanent installation requires a Mozilla-signed XPI.'
  fi
  printf '  %s\n' "$package/sdl-firefox.xpi"
fi
if [ "$platform" = Darwin ]; then
  if [ "$family" = firefox ] && [ "$page" = "$package/sdl-firefox.xpi" ]; then
    open -a "$browser" "$page"
  else
    open -a "$browser" --args "$page"
  fi
else
  "$browser" "$page" >/dev/null 2>&1 &
  # Wait only for the launch request; browsers normally forward it to their existing process.
  # Do not wait for a newly opened browser to close.
  sleep 1
fi
echo 'No extension downloads or browser policy changes were requested.'
if "$pause" && [ -t 0 ]; then printf 'Press Enter to close setup: '; read -r answer; fi
