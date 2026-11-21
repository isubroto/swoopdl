"""Additional first-class website pages and practical help guides."""
from content import FEATURES, HELP
from localization import languages


GUIDES = {
    'guide-first-download': ('Your first download.', 'From a link to a file, in three steps.', [
        ('Copy the original link', 'Find a direct file link from a source you trust. Links that expire or require a sign-in may need to be refreshed.'),
        ('Add it to Swoopdl', 'Choose Add URL, paste the address, and review the suggested file name and destination. Make sure the destination has enough disk space.'),
        ('Start and follow the progress', 'Start the transfer and watch its progress in the download list. Use pause and resume where the source server supports them.')]),
    'guide-catch-browser-downloads': ('Bring your browser along.', 'Enable capture only after approving the local extension.', HELP['browser-integration'][2]),
    'guide-download-overnight': ('Leave it for the night.', 'Build a queue that fits your schedule.', HELP['queues-and-scheduler'][2]),
    'guide-keep-speed-for-everything-else': ('Leave a little bandwidth.', 'Keep a call or another application comfortable.', HELP['connections-and-speed'][2]),
    'guide-resume-broken-download': ('Pick up where you left off.', 'Give an interrupted file another chance.', [
        ('Keep the partial file', 'Do not delete or move the partial download and its saved state. Swoopdl uses that information to continue a supported transfer.'),
        ('Check the original source', 'Confirm that the original link still works and the file has not changed. Refresh an expired link using the original source.'),
        ('Choose resume', 'Select the download and resume it. If the server cannot resume or the source changed, you may need a new transfer.')]),
    'guide-find-in-long-list': ('Find that one file.', 'Keep a long download list useful.', [
        ('Use the list search', 'Search for a recognizable part of the file name. Check categories and completion state if the file is not immediately visible.'),
        ('Organize related files', 'Use categories and consistent destination folders for related work. Remove finished entries only after checking that the files are saved where you expect.')]),
    'guide-usb-stick': ('Take your downloads with you.', 'A few checks before using a portable folder.', [
        ('Keep the application together', 'If you have a portable build, keep its application files together in a writable folder. Leave enough space for downloads and saved state.'),
        ('Check browser setup separately', 'Browser integration needs local native messaging registration and browser approval on each computer. Skip browser setup if you want to avoid those changes.'),
        ('Pause before unplugging', 'Stop active transfers and quit Swoopdl before safely ejecting a removable drive.')]),
    'guide-video-from-page': ('Save a supported video.', 'Choose an available format from the original page.', [
        ('Paste a page link', 'Copy the address of a supported video page and add it in Swoopdl. Only download content you are allowed to save.'),
        ('Choose a format', 'Review the formats and quality levels that the source makes available. Video support uses yt-dlp, with FFmpeg for supported merging tasks.'),
        ('Let the tools finish', 'Helper tools may need to be downloaded on first use. Keep enough space for video, audio, and their merged output.')]),
    'keyboard-shortcuts': ('Keep your hands on the keys.', 'Move through the application with the keyboard.', [
        ('Navigate controls', 'Use Tab and Shift+Tab to move between controls. Use Enter or Space to activate a focused button and Escape to dismiss supported dialogs.'),
        ('Check menus for shortcuts', 'Application menus show the shortcuts available in your build. Operating-system shortcuts and screen-reader behavior can vary by platform.')]),
}

HELP_FEATURES = {
    'adding-downloads': 'batch', 'download-list': 'categories', 'video': 'video', 'torrents': 'torrents',
    'torrent-security': 'torrents', 'options': 'dns-proxies', 'portable': 'systems',
    'browser-help': 'browser-extension', 'themes': 'themes', 'languages': 'languages',
    'link-grabber': 'link-grabber', 'remote-control': 'remote', 'server': 'server',
    'galleries': 'galleries', 'debrid': 'debrid', 'crash-reports': 'crash-reports',
    'ai-assistants': 'ai-assistants', 'command-line': 'command-line',
}


def extra_pages(write, article, icon, esc, root, source_version):
    intro = '<section class="page-hero container"><span class="section-kicker">A space that feels like you</span><h1>Your desktop.<br>Your atmosphere.</h1><p class="lead">A selection of application themes, with light and dark choices. Find your favorite way to settle in.</p></section>'
    samples = [('classic', 'Classic', 'Familiar controls. A clear starting point.'), ('night', 'Dark', 'A quieter setting for a late-night queue.'), ('contrast', 'High contrast', 'Strong separation and clear boundaries.'), ('breeze', 'Breeze', 'Cool blues and room to breathe.'), ('aero', 'Aero', 'A little color with a familiar feel.'), ('aqua', 'Aqua', 'Soft surfaces and an ocean accent.')]
    cards = ''.join(f'<article class="theme-card"><div class="theme-sample sample-{kind}" aria-hidden="true"><div class="sample-bar"><i></i><i></i><i></i></div><div class="sample-body"><aside></aside><div><b></b><span></span><span></span><span></span></div></div></div><div class="theme-description"><h2>{esc(name)}</h2><p>{esc(copy)}</p></div></article>' for kind, name, copy in samples)
    write('/themes.html', 'Make yourself at home', 'Explore Swoopdl application themes and appearance options.', intro + '<section class="container section-bottom"><div class="theme-grid">' + cards + '</div><p class="illustration-note">Theme illustrations are original website concepts. Actual controls depend on your operating system and build.</p></section>')
    language_cards = ''.join(f'<a class="language-card" data-language-choice="{item["code"]}" href="?lang={item["code"]}" lang="{item["code"]}" dir="auto" translate="no"><strong>{esc(item["native"])}</strong><span>{esc(item["english"])}</span><small>{esc(item["code"])}</small></a>' for item in languages())
    write('/languages.html', 'A language that feels familiar', 'Choose from the same language list as the Swoopdl application.', '<section class="page-hero container"><span class="section-kicker">Made for more of us</span><h1>A familiar language.<br>Wherever you are.</h1><p class="lead">Choose from the same 86 languages as the application. Your choice follows you across the website.</p><p class="development-note">Website translations are being expanded. Any untranslated passage is marked as English.</p></section><section class="container section-bottom"><div class="language-directory">' + language_cards + '</div></section>')
    write('/news.html', 'Development status', 'Current Swoopdl development and package availability.', article('A new chapter is taking shape.', 'Development status', [('Current source', f'The source checkout is at development version {source_version}. This is a source version, not a promise of a published package.'), ('Package availability', '[[PACKAGE_HELP]] Visit the download page for current platform availability and local browser setup instructions.'), ('What comes next', 'The website will list real packages and release details when they are available. No release dates or performance multipliers are promised.')], label='Swoopdl'))
    write('/contributing.html', 'Make Swoopdl better', 'Ways to help with source, testing, and translations.', article('A little help makes a difference.', 'Contributing to Swoopdl', [('Start with your checkout', 'Read the project README and CONTRIBUTING instructions in the source checkout. Build on your target operating system and keep a note of the version you tested.'), ('Help with language', 'Review an application translation or a website catalog. Keep product names, links, technical identifiers, and the meaning of safety instructions intact.'), ('Make a useful report', 'Describe the expected behavior, what happened instead, the operating system, and the steps to reproduce. Remove private addresses and credentials before sharing.')], label='Swoopdl'))
    write('/contributors.html', 'Built on shared work', 'Swoopdl credits and open-source foundations.', article('Shared work. A new direction.', 'Credits and contributors', [('Software authorship', 'Copyright © 2026 Ahmed Nefzaoui and contributors. Existing authorship, license notices, and third-party obligations remain part of the source.'), ('Swoopdl identity', 'This website uses the approved Swoopdl blue and teal artwork from the project. Its interface illustrations and flowing background artwork are original website assets.'), ('Open-source foundations', 'The software and website rely on open-source work. The license page provides the software license, font license, and third-party list.')], label='Swoopdl', extra='<a class="button" href="/license.html">Open-source licenses ' + icon('arrow') + '</a>'))
    guides = dict(GUIDES)
    for slug, feature_slug in HELP_FEATURES.items():
        feature = next(row for row in FEATURES if row[0] == feature_slug)
        guides[slug] = (feature[1], feature[2], [('Use this feature', feature[4]), ('Keep the basics in view', 'Check your source link, destination folder, free disk space, and any optional integration settings before starting a large task.')])
    for doc in (root / 'docs').glob('*.md'):
        slug = doc.stem
        if slug not in guides and slug not in HELP:
            title = ' '.join(word.capitalize() for word in slug.replace('_', '-').split('-'))
            guides[slug] = (title, 'A guide to the Swoopdl source checkout.', [('Start with the project', 'Read the README and this guide in your local source checkout for implementation details. Source documentation stays with the checkout.'), ('Build and validate', 'Use the toolchain specified by the checkout. Build and test on the target operating system, and keep source changes separate from generated output.')])
    for slug, (title, summary, blocks) in guides.items():
        write(f'/help/{slug}.html', title, summary, article(title, summary, blocks))
