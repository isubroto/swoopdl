#!/bin/bash
# Host side (Git Bash on Windows): one SDL window in the lab VM, cropped from VirtualBox's own
# screenshot (capturing an OpenGL window from inside the guest gives an empty image).
#   shot-host.sh <vm> <label> <main|fileinfo|show name> <lang> <light|dark> <out.png>
# SDL_STATE (a saved downloads.json in the guest) and SDL_SEEN (extension-seen lines) pass through
# to shots.sh. SDL_VM_USER / SDL_VM_PASSWORD as for vm.ps1.
V="${VBOXMANAGE:-/c/Program Files/Oracle/VirtualBox/VBoxManage.exe}"
user=${SDL_VM_USER:-sdl}; pass=${SDL_VM_PASSWORD:-}
keys="${USERPROFILE:-$HOME}/.sdl-dev/vm-account.txt"
if [ -z "$pass" ] && [ -f "$keys" ]; then
  pass=$(sed -n 's/^[[:space:]]*password:[[:space:]]*//p' "$keys" | head -1 | tr -d '')
  [ -n "${SDL_VM_USER:-}" ] || user=$(sed -n 's/^[[:space:]]*user:[[:space:]]*//p' "$keys" | head -1 | tr -d '')
fi
[ -n "$pass" ] || { echo "no VM account: set SDL_VM_USER and SDL_VM_PASSWORD" >&2; exit 1; }
vm=$1; out=$6
geo=$(MSYS_NO_PATHCONV=1 "$V" guestcontrol "$vm" run --exe /bin/bash --username "$user" --password "$pass" --timeout 150000 --wait-stdout -- \
  -c "SDL_STATE=${SDL_STATE:-} SDL_SEEN='${SDL_SEEN:-}' bash /home/$user/lab/shots.sh $2 $3 $4 $5" 2>/dev/null | tail -1)
full="${out%.png}.full.png"
"$V" controlvm "$vm" screenshotpng "$(cygpath -w "$full")"
python - "$full" "$out" $geo <<'PY'
import sys
from PIL import Image
full, out, x, y, w, h = sys.argv[1], sys.argv[2], *map(int, sys.argv[3:7])
Image.open(full).crop((x, y, x + w, y + h)).save(out, optimize=True)
print(out, x, y, w, h)
PY
rm -f -- "$full"
