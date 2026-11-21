#!/bin/sh
# Prefer the installed app so native messaging never points at a mounted disk image.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ -f "$here/install-local.sh" ]; then
  exec /bin/sh "$here/install-local.sh" "$@"
fi
for app in /Applications/Swoopdl.app "$HOME/Applications/Swoopdl.app"; do
  if [ -f "$app/Contents/Resources/extension/install-local.sh" ]; then
    exec /bin/sh "$app/Contents/Resources/extension/install-local.sh" "$@"
  fi
done
echo 'Copy Swoopdl.app to Applications first, then run this setup again.' >&2
printf 'Press Enter to close: '
read -r answer
exit 1
