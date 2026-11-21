# The browser and SDL aren't connected

[← Back to docs index](README.md)

When the browser can't hand a download to SDL, it keeps the download itself, so nothing is lost. The extension's toolbar button then shows a red **!**. Click it to see what's wrong, then **How to fix it** brings you to the matching section below.

To check SDL's side, open **Options → General** in SDL and click **Test browser integration...**. It checks that browser integration is on, that this is the installed copy, that the browsers can find SDL, and which browsers' extension has connected; **How to fix it** opens the section for the first that fails.

## SDL isn't installed, or browser integration isn't set up {#not-installed}

The browser cannot find the bundled native messaging helper, or registration points at an old installation.

1. **Windows:** run **Swoopdl Browser Setup** from the Start menu. If the shortcut is missing, rerun the installer with browser integration selected.
2. **Linux:** run `sh /usr/share/swoopdl/extension/install-local.sh` after installing the .deb/.rpm. For an extracted archive, use its bundled helper.
3. **macOS:** copy Swoopdl.app to Applications and run **Set up browser extension.command** from the disk image.
4. **AppImage:** extract it to a permanent folder before setup. A native host registered inside a temporary mount will stop working when that mount disappears.
5. **Windows ZIP packages:** run the bundled `extension\install-local.ps1` explicitly. This writes user registration entries; skip it to keep portable use registry-free.
6. Confirm Swoopdl's browser integration option is enabled, then restart the browser.

Use the files from the same Swoopdl package. See [Browser integration](browser-integration.md#set-it-up) for exact commands and browser approval steps.

## SDL didn't start in time {#not-running}

When SDL isn't running, the browser starts it for each download and waits for it. It waited about 20 seconds this time and gave up.

- Start SDL yourself and try the download again.
- A first start, a slow disk, or an antivirus scanning SDL can take that long. Next time it is usually quick.
- With a VPN or proxy program, allow connections to `127.0.0.1` (your own computer): the browser's helper reaches SDL through one.
- If SDL never opens, see [The setup or SDL won't start](troubleshooting.md).

## The connection to SDL failed {#host-failed}

The browser started SDL's helper, but it stopped before answering. Security software is the usual cause.

- Allow `swoopdl-nmhost.exe` (in SDL's folder) in your antivirus or anti-malware program.
- Reinstall SDL to restore its files.
- On Linux, with Firefox or Chromium installed as a Snap, see [Flatpak and Snap browsers](#linux-sandbox).
- Uninstalled SDL on Linux? The .deb and .rpm take their browser files with them; the AppImage's and the tar.gz's stay in your home folder: [remove them](getting-started.md#uninstall-linux), and the extension too.

## SDL doesn't accept this copy of the extension {#forbidden}

SDL's helper serves only its own extension. The browser says this copy isn't one it knows.

- Remove extensions from other projects or old Swoopdl installations.
- Load the `chromium` folder or Firefox XPI bundled with the same Swoopdl version.
- Run the local setup helper again if you moved or replaced the application.
- Keep only one Swoopdl extension enabled in each browser.

## The extension is newer than SDL {#update-app}

Use Swoopdl and the extension files from the same package version. Install the matching application package, then reload the unpacked Chromium extension or load its Firefox XPI again.

Bundled extensions do not update through a browser store. If your development build has no configured update feed, obtain the matching package from the Swoopdl maintainer.

## A download's details were too large {#too-large}

A download carries its site's cookies (and a form's data) to SDL. A few sites set so many that together they exceed what SDL accepts from the browser (1 MB). The browser kept this one. Signing out and back in to the site usually trims them.

## Downloads stay with the browser although SDL is connected {#options}

The extension is connected, but SDL's own settings leave this download to the browser. Check, in SDL:

1. **Options → General → Use advanced browser integration** is on, and your browser is ticked under **Capture downloads from the following browsers**.
2. The file's type is listed in **Options → File types**.
3. The site isn't in **Don't start downloading automatically from the following sites**, nor the address in **Don't start downloading automatically from the following addresses**.
4. The file isn't smaller than **Don't take files smaller than** in **Options → File types**.

Also check the extension's toolbar button: with **Take downloads from this browser** off, everything stays with the browser.

## The browser opens PDFs and music instead of sending them {#opened}

SDL also takes the file types the browser would show or play itself (a PDF, an MP3), when they're listed in **Options → File types** and **Also take files the browser would open** is ticked in **Options → General → Keys...**. This needs a Chromium browser (Chrome, Edge, Brave, Opera, Vivaldi) version 128 or later, or Firefox. Sites in SDL's excluded sites keep opening them in the browser. Hold **Alt** while clicking a link to leave it to the browser: Firefox then opens it, while Chromium browsers download it themselves (Alt+click is their own "download link").

## No Download video button on YouTube in Chrome, Edge or Brave {#store-youtube}

Check that the bundled extension is enabled and allowed to run on the page. Reload it after an update, then reload the video page. Some sites restrict what extensions can inspect.

You can also use **Tasks → Download video from a page** in Swoopdl and paste the page address. Availability depends on the site and the configured video helper.

## The extension can't read this site {#site-access}

In Chrome, Edge and Brave you can limit an extension to some sites (right-click its toolbar button → **This can read and change site data**). On a site it may not read, the extension can't pass your sign-in on, so it leaves the download to the browser. Choose **On all sites** to let SDL take downloads everywhere.

## Firefox forgets the extension after a restart {#firefox-temporary}

An unsigned add-on loaded with **Load Temporary Add-on** is removed when Firefox closes. Load the bundled `sdl-firefox.xpi` again for another development session.

Permanent installation in standard Firefox requires a Mozilla-signed XPI. If a signed Swoopdl package is supplied, install that local XPI through **Install Add-on From File** in `about:addons`. Do not use another project's store listing.

## Linux: Flatpak and Snap browsers {#linux-sandbox}

Browsers installed as a Flatpak or a Snap run in a sandbox, which decides whether they can reach SDL.

- **Firefox as a Snap** (Ubuntu's own Firefox) reaches SDL through the desktop, which reads the browser files in your home folder and those the .deb or .rpm installs for everyone. The first time the extension calls SDL, the desktop asks **Allow Firefox to start WebExtension backend?** for Swoopdl: choose **Allow**. After **Don't allow**, the extension says the connection failed and the desktop doesn't ask again; to be asked again, run this in a terminal, then use the extension again:

  `gdbus call --session --dest org.freedesktop.impl.portal.PermissionStore --object-path /org/freedesktop/impl/portal/PermissionStore --method org.freedesktop.impl.portal.PermissionStore.Delete webextensions com.swoopdl.host`
- **Chromium as a Snap** (Ubuntu's Chromium) starts SDL's helper inside its sandbox, where it can't reach SDL, so SDL doesn't list it in **Add Swoopdl to your browser**. Use Firefox, or Google Chrome from its own `.deb`.
- **Flatpak browsers** (such as Zen's `app.zen_browser.zen`) can't start SDL's helper, so SDL doesn't list them either. Install the browser from your distribution's packages (or the browser maker's own `.deb`, `.rpm`, tarball or AppImage) to use browser integration.

## Next steps

- [Browser integration →](browser-integration.md)
- [Troubleshooting →](troubleshooting.md)
- [Back to the docs index →](README.md)
