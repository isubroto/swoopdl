# Portable mode

[← Back to docs index](README.md)

The portable copy of SDL runs from any folder, including a USB stick, without installing anything.

## How it works

`Swoopdl-<version>-portable.zip` holds the same programs as the setup, plus a folder named `data` next to `swoopdl.exe`. When that folder is there and SDL can write to it, SDL is in portable mode:

- The download list, settings, helpers for video and galleries (yt-dlp, ffmpeg, gallery-dl), torrent data, your [theme files](themes.md) (`data\themes`) and the logs are kept in `data`, not in your Windows profile.
- Passwords (Sites Logins, the proxy's, debrid accounts) stay in `data` too, instead of in Windows Credential Manager. They are sealed for your Windows account: on another computer, or another account, type them again (and sign in to a debrid account again).
- Normal portable startup does not register browser integration. Explicit browser setup and file-association actions can write user registry entries; see below.

Delete the `data` folder (or make it read-only) and the same copy behaves like the plain zip: it keeps its data in `%APPDATA%\swoopdl`.

## Set it up

1. Download `Swoopdl-<version>-portable.zip` from the Swoopdl package source provided by its maintainer. Do not use another project's releases.
2. Unzip it where you want SDL to live, for example `E:\SDL` on a USB stick.
3. Run `swoopdl.exe` from there.

On the first start SDL asks for the language; see [Languages](languages.md).

## What is different

- **Downloads still go to the computer's Downloads folder** unless you change it. To keep them with SDL, set each category's folder in **Options → Save to**. [Carry SDL on a USB stick](guide-usb-stick.md) shows how.
- **Launch Swoopdl on startup** in **Options → General** is greyed: starting with Windows needs the registry.
- **Browser integration is optional and explicit.** The archive includes an `extension` folder. Running its `install-local.ps1` helper registers the adjacent native host in your Windows user registry and guides local extension loading. Keep that folder and drive letter stable. To stay registry-free, skip this setup and use **Add URL**, the clipboard, or the drop target instead (see [Adding downloads](adding-downloads.md)).
- **Tasks → Torrent settings → Open magnet links and .torrent files with Swoopdl** does write to the registry, for your Windows account only. Leave it off to keep the portable copy registry-free.

## Moving between computers

The list keeps each download's full folder path. If Windows gives the USB stick a different drive letter on another computer, check **Options → Save to**, and expect downloads saved under the old letter to be missing until the letter matches again. When SDL is about to save into a folder that doesn't exist, it asks whether to create it.

Quit SDL with **Tasks → Exit** before you unplug the stick, so it saves the list.

## Next steps

- [Tutorial: carry SDL on a USB stick →](guide-usb-stick.md)
- [Getting started →](getting-started.md)
- [Options → Save to →](options.md#save-to)
