# Swoopdl — documentation

Swoopdl (SDL) is a free download manager for Windows and Linux (and, still experimental, macOS). It is laid out the way Internet Download Manager users expect: a toolbar, categories on the left, one list of downloads and a browser extension that hands downloads to it.

These pages explain each part of the program. The tutorials walk through the things people do most, one task each.

Current behavior is checked against the implementation, not inferred from changelog entries. The [source map](SOURCE_GUIDE.md) identifies the code, defaults, and packaging scripts behind key instructions. See [Contributing](../CONTRIBUTING.md) for validation commands.

![The main window](images/main-light.webp)

## Start here

- **[Getting started](getting-started.md)** — install SDL (setup, portable or zip on Windows; the packages on Linux; the .dmg on a Mac), choose a language and find your way around the main window.
- **[Your first download](guide-first-download.md)** — a two-minute walk-through from copied link to opened file.

## Downloading

- **[Adding downloads](adding-downloads.md)** — Add URL, the clipboard, the drop target, batch downloads and lists of links, and Start Download versus Download Later.
- **[Link grabber and archives](link-grabber.md)** — many links at once, in packages; zip, 7z and rar archives extracted when they are downloaded.
- **[Site Grabber](site-grabber.md)** — saved static website captures, offline copies, pause/resume, reports, and scheduled synchronization.
- **[Browser integration](browser-integration.md)** — bundled local setup for Windows, Linux, and macOS, Chromium loading, Firefox signing limits, and download capture settings.
- **[Run SDL on a server](server.md)** — SDL without a window on a home server or NAS (Docker, Unraid), downloading into its folders; send it downloads from your phone or browser.
- **[Control SDL from your phone](remote-control.md)** — add links, magnets and torrents from a phone or another computer at home, and watch and control the list.

## Organising

- **[The download list](download-list.md)** — columns, filters, the categories tree, selecting many downloads, finding one, and deleting downloads with or without their files.
- **[Queues and the Scheduler](queues-and-scheduler.md)** — run downloads a few at a time, start and stop them by the clock, and turn the computer off when they are done.

## Speed and connections

- **[Connections and speed](connections-and-speed.md)** — the progress window, how SDL splits a file over several connections, resuming, and the Speed Limiter.

## Video and torrents

- **[Video downloads](video.md)** — download a video from its page (YouTube, Vimeo and many more) with yt-dlp.
- **[Image galleries](galleries.md)** — download a gallery's pictures (Wikimedia Commons, Flickr, DeviantArt and many more) with gallery-dl.
- **[Torrents](torrents.md)** — magnet links and .torrent files, choosing files, and sharing.
- **[Torrents and your security](torrent-security.md)** — what others can see, the network switches, blocklists, proxies.
- **[Debrid service accounts](debrid.md)** — Real-Debrid, AllDebrid and Premiumize: file-host links through your own account.

## Settings

- **[Options](options.md)** — every page of the Options window.
- **[Themes](themes.md)** — the built-in looks (Classic, Native, High contrast, Metro, Adwaita, Breeze, Luna, Aero, Aqua), and how to make your own in a text file.
- **[Languages](languages.md)** — the 86 interface languages, numbers and dates, right-to-left layouts, and how to help translate.
- **[Portable mode](portable.md)** — run SDL from a folder or a USB stick without installing it.
- **[Keyboard shortcuts](keyboard-shortcuts.md)** — every key the main window answers, and screen readers.
- **[Command line and automation](command-line.md)** — IDM's command-line switches, the `swoopdl-cli` tool, and the automation API other programs can use.
- **[AI assistants (MCP)](ai-assistants.md)** — let Claude, Cursor, VS Code or another MCP assistant list, add, pause and remove your downloads.

## Tutorials

- **[Download a file](guide-first-download.md)** — copy a link, add it, open the file.
- **[Catch downloads from your browser](guide-catch-browser-downloads.md)** — set up the extension so clicked downloads go to SDL.
- **[Download overnight](guide-download-overnight.md)** — queue files and let the Scheduler start them at night.
- **[Resume a broken download](guide-resume-broken-download.md)** — continue after a cut connection, a crash or an expired link.
- **[Keep some speed for everything else](guide-keep-speed-for-everything-else.md)** — cap SDL so browsing and calls stay smooth.
- **[Download a video from a page](guide-video-from-page.md)** — pick a quality and save a video.
- **[Carry SDL on a USB stick](guide-usb-stick.md)** — the portable copy, with downloads saved to the stick.
- **[Find a download in a long list](guide-find-in-long-list.md)** — Find, Find Next and column filters.

## Help

- **[Crash reports](crash-reports.md)** — what the crash report window shows when SDL closes unexpectedly or stops responding, what a report contains, and how to send it.
- **[Troubleshooting](troubleshooting.md)** — what to check when a download fails, the browser doesn't hand over a download, or a schedule doesn't start.
- **[The browser and SDL aren't connected](browser-help.md)** — what the extension's red **!** means, reason by reason, and how to fix each.
- **[Getting started](getting-started.md#choose-a-download)** — choose a Swoopdl package from the source provided by its maintainer.
- **Report a problem through this repository's Issues page** — or use **Help → Report a problem** in SDL, which fills in your version, system and how SDL was installed for you.

---

> Screenshots in these docs show Windows and the light theme unless noted; on the website each has its dark version beside it. SDL follows the system's dark mode: when Windows (or your desktop) is set to dark, SDL is dark too.
