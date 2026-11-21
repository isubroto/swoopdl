"""Curated website copy, independent of old screenshots and release announcements."""

# Slug, title, summary, icon, practical detail.
FEATURES = [
    ('acceleration', 'More connections. Less waiting.', 'Split supported downloads across parallel connections and keep each part moving.', 'zap', 'Swoopdl learns whether a server supports range requests. When it does, multiple connections share the file. If it does not, the download uses a single connection. Actual speed depends on your connection and the server.'),
    ('torrents', 'Torrents, right alongside files.', 'Open magnet links and torrent files without switching to another app.', 'torrent', 'Choose the files you want, follow the peers and pieces, and control seeding from the same download manager. Only download and share material you have permission to use.'),
    ('video', 'A link becomes a video.', 'Paste a supported page link, then choose an available format and quality.', 'video', 'Video support uses yt-dlp and FFmpeg. Available formats depend on the source. Helper tools may be downloaded on first use; Swoopdl verifies managed downloads before running them. Protected or unsupported content may not be available.'),
    ('scheduler', 'On your schedule.', 'Organize queues and choose when downloads should start and stop.', 'clock', 'Create a queue for overnight downloads or a quieter part of your day. Review scheduled actions before enabling shutdown after a queue finishes.'),
    ('browser-extension', 'From browser to download list.', 'Bring supported browser downloads into Swoopdl through local integration.', 'browser', 'Extension files are bundled with application packages. Chromium browsers need Developer mode and Load unpacked. The unsigned Firefox extension is temporary and is removed after restart. Capture is off by default; enable it in Options after setup.'),
    ('categories', 'Everything in its place.', 'Keep documents, archives, videos, and other files organized.', 'folder', 'Use categories and destination folders to keep your list useful. Check the save location when adding a download, and keep enough free space for the full file.'),
    ('checksums', 'Check what you downloaded.', 'Compare a file with a checksum published by its original source.', 'shield', 'Use Verify checksum from the download list and paste the value provided by the publisher. A match confirms that the file matches that checksum; it does not establish that the publisher or file is safe.'),
    ('languages', 'A familiar language.', 'The application includes 86 interface languages and right-to-left layouts.', 'globe', 'Choose the application language during setup or in its language options. Arabic, Persian, Hebrew, and Urdu use right-to-left layouts. Use the website language picker to choose your preferred language. Untranslated passages are marked as English.'),
    ('speed-limiter', 'Leave room for everything else.', 'Set a download speed limit when the rest of your connection needs space.', 'sliders', 'Use the speed limiter for calls, streaming, or a shared connection. Connection count and speed limits are separate controls; increasing connections cannot make a server exceed its own limit.'),
    ('batch', 'Add a whole batch.', 'Bring multiple download links into your list together.', 'plus', 'Review the links and their destination before starting a batch. The same pause, resume, and queue controls apply to supported individual downloads.'),
    ('themes', 'Make yourself at home.', 'Choose light, dark, or another built-in application theme.', 'sun', 'The desktop application includes theme and interface-size controls. The website has its own light and dark setting, remembered only in your browser.'),
    ('accessibility', 'Room for your way of working.', 'Use keyboard controls and application accessibility support.', 'sliders', 'Keyboard and screen-reader behavior depends on the operating system and build. The website provides visible focus, keyboard navigation, and reduced-motion support.'),
    ('archives', 'Unpack the next step.', 'Work with supported archive downloads and extraction.', 'folder', 'Check the extraction destination and available disk space. Password-protected or unsupported archives may need additional input or another tool.'),
    ('link-grabber', 'Find the files behind a page.', 'Collect downloadable links and review what to add.', 'search', 'Review collected links before sending them to your download list. Site restrictions, sign-ins, and generated links can affect what can be discovered.'),
    ('galleries', 'Keep a gallery together.', 'Download supported image galleries using gallery-dl integration.', 'folder', 'Gallery support depends on the site and gallery-dl. Helper tools and sign-in information may be needed. Only save material you have permission to download.'),
    ('rss', 'Follow torrent feeds.', 'Use RSS feeds to discover torrent entries.', 'torrent', 'Review your feed and filtering settings carefully. Automatic additions should use sources you trust and destinations with enough disk space.'),
    ('dns-proxies', 'Choose your route.', 'Configure proxies and DNS for supported downloads.', 'globe', 'Use connection settings for proxy and DNS options. A proxy or custom DNS service has its own privacy and availability characteristics.'),
    ('network-adapter', 'Use the connection you choose.', 'Bind supported traffic to a selected network adapter.', 'globe', 'Review adapter settings when using multiple connections or a VPN. Platform behavior and selected routing options determine which downloads are bound.'),
    ('remote', 'Control from another device.', 'Use the optional remote interface on your local network.', 'browser', 'Remote access is off until enabled. Pair only devices you trust, keep access credentials private, and avoid exposing the service directly to the public internet.'),
    ('server', 'Run without a desktop.', 'A headless mode supports server-based download workflows.', 'terminal', 'Headless and container operation require configuration. Review storage paths, permissions, network exposure, and authentication before starting the service.'),
    ('command-line', 'A place in your workflow.', 'Use command-line tools for repeatable download tasks.', 'terminal', 'Build the CLI from the source checkout and run its help command for the options available in that build. Keep URLs and credentials out of shared shell history.'),
    ('ai-assistants', 'Connect your own tools.', 'Automation interfaces support integrations with other applications.', 'terminal', 'The source includes automation and MCP support. Enable only the interfaces you need and give connected tools only the access appropriate to your workflow.'),
    ('crash-reports', 'You choose what to share.', 'Review diagnostic information when something goes wrong.', 'shield', 'The application does not automatically send crash reports. Review report contents before sharing. Remove private links and credentials from any report.'),
    ('debrid', 'Bring your supported account.', 'Connect supported debrid services through application settings.', 'link', 'Service availability, account requirements, subscriptions, and limits are set by each provider. Add only accounts you own and review the provider terms.'),
    ('tray', 'Keep the desktop clear.', 'Manage background downloads from the system tray where supported.', 'download', 'Closing a window and quitting the application can behave differently. Review the application tray options before leaving downloads unattended.'),
    ('systems', 'Made for the desktop.', 'Windows and Linux support, with an experimental macOS build.', 'windows', 'See the download page for package availability. Build on the target operating system and validate behavior there. macOS support is experimental.'),
]

FAQ = [
    ('Is Swoopdl free?', 'Yes. Swoopdl is free, open-source software licensed under GPL-3.0-or-later. There is no paid edition or trial countdown.'),
    ('Where can I download it?', '[[DOWNLOAD_ANSWER]]'),
    ('Which operating systems does it support?', 'The application is developed for Windows and Linux. macOS support is experimental. Availability and testing can differ between builds.'),
    ('Does browser integration work automatically?', 'No. Run the local browser setup, approve the extension, and then enable advanced browser integration in Swoopdl. Capture is off by default.'),
    ('Can I resume an interrupted download?', 'For servers that support resuming, Swoopdl saves download state so work can continue after an interruption. The source file must still be available and unchanged.'),
    ('Does this website track me?', 'No analytics or third-party tracking scripts are included. Your theme preference is stored locally in your browser. A future hosting provider may keep ordinary server logs.'),
]

HELP = {
    'getting-started': ('A good place to start.', 'Get ready for your first download.', [
        ('Get the application', '[[PACKAGE_HELP]] If you already have a source checkout, follow its README to build Swoopdl on your operating system.'),
        ('Add your first link', 'Open Swoopdl and choose Add URL. Paste the direct file link, review the destination and file name, then start the download.'),
        ('Stay in control', 'Use pause and resume for supported downloads. Organize related files in a queue, or set a speed limit to leave room for other applications.')]),
    'browser-integration': ('Your browser. Your choice.', 'Set up browser integration locally, with your approval.', [
        ('Start with the local helper', 'Application packages include extension files. Run the browser setup helper provided with your package. It registers the native messaging connection; it does not install a store extension automatically.'),
        ('Chromium browsers', 'Open the browser extensions page, enable Developer mode, choose Load unpacked, and select the bundled chromium folder. Approve the requested permissions.'),
        ('Firefox', 'Load the bundled unsigned sdl-firefox.xpi temporarily through about:debugging. Firefox removes temporary extensions after restart. Permanent installation in standard Firefox requires a Mozilla-signed extension.'),
        ('Enable capture when ready', 'In Swoopdl, open Options, then General. Enable Use advanced browser integration and select the browsers you want. Capture starts off disabled. Safari integration is not included.')]),
    'connections-and-speed': ('Find your comfortable speed.', 'Understand parallel connections, speed limits, and resume.', [
        ('Parallel connections', 'The engine can split a supported file into parts. A server that ignores range requests uses a single connection instead. More connections do not guarantee more speed.'),
        ('Share your bandwidth', 'Set a speed limit when you need room for calls or other applications. A remote server can also limit your transfer rate.'),
        ('Resume with care', 'Keep the partial file and its saved state together. Resuming depends on server support and the original file remaining available and unchanged.')]),
    'queues-and-scheduler': ('Downloads that fit your day.', 'Group files and choose a convenient time to download.', [
        ('Create a queue', 'Put related downloads in a queue and choose how many should run at once.'),
        ('Choose your window', 'Configure start and stop times in the scheduler. Leave Swoopdl running and make sure the computer stays awake.'),
        ('Review finishing actions', 'If you enable shutdown or another finishing action, check that it will not interrupt other work.')]),
    'troubleshooting': ('Get moving again.', 'A few checks for a download that needs attention.', [
        ('Check the link', 'Open the original source in your browser. Download links can expire or require a sign-in. Refresh the link if needed.'),
        ('Check the destination', 'Make sure the destination exists, is writable, and has enough space. Keep partial files if you intend to resume.'),
        ('Check integration', 'If browser capture does not work, confirm that the local helper ran, the extension is approved, and advanced browser integration is enabled in Swoopdl.'),
        ('Keep useful details', 'Note the operating system, source version, error message, and steps to reproduce. Remove private links and credentials before sharing a report.')]),
}

LEGAL = {
    'privacy': ('Your downloads are your business.', 'Privacy', [
        ('This website', 'This website serves its fonts, scripts, and images locally. It includes no analytics, advertising, or tracking scripts. Theme preference is saved in your browser local storage and is not sent to a server.'),
        ('The application', 'The application makes network requests needed for downloads and enabled features. Sources, torrent peers, configured services, and helper-tool providers may receive the connection information needed for those requests. Review a provider privacy policy before using it.'),
        ('Reports and hosting', 'Crash reports are not automatically uploaded. Review diagnostic information before sharing it. A hosting provider may keep ordinary access logs.')]),
    'security': ('A little care goes a long way.', 'Security', [
        ('Get files from their source', 'Download software from a source you trust and compare checksums when available. A matching checksum confirms file integrity, not that a file is harmless.'),
        ('Protect your accounts', 'Keep service credentials and private download links private. Pair only trusted devices with optional remote access and avoid exposing local services publicly.'),
        ('Report responsibly', 'A public Swoopdl security contact has not been configured yet. Do not post credentials, private links, or sensitive vulnerability details in public channels.')]),
    'terms': ('Use it thoughtfully.', 'Terms of use', [
        ('Your use', 'Download and share only material you have permission to access. Follow applicable rules and the terms of the services you use.'),
        ('Availability', 'The application is under development. Features and platform behavior can change. This website does not promise a particular transfer speed or availability of a third-party service.'),
        ('License and warranty', 'The software is licensed under GPL-3.0-or-later and is provided without warranty as described in its license. This page does not replace that license.')]),
    'intended-use': ('Built for files you can access.', 'Intended use', [
        ('Everyday downloads', 'Use Swoopdl to manage documents, archives, media, software, and other files you are permitted to download.'),
        ('Respect the source', 'Do not use the application to bypass access restrictions or distribute material without permission. Third-party services may impose additional conditions.')]),
}
