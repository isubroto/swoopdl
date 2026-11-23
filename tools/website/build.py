#!/usr/bin/env python3
"""Build the Swoopdl website offline. Run with --serve for a local preview.

Only curated website content and approved assets are published. Optional production
URLs and release files live in website/site.json; no API credentials are needed.
"""
from __future__ import annotations

import argparse
import functools
import hashlib
import html
import http.server
import json
from pathlib import Path
import re
import shutil
import sys
import tomllib
from urllib.parse import urlsplit
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
WEB = ROOT / 'website'
OUT = ROOT / 'target/site'
sys.path.insert(0, str(Path(__file__).resolve().parent))
from content import FEATURES, FAQ, HELP, LEGAL
import localization
from pages import extra_pages

SOURCE_VERSION = tomllib.loads((ROOT / 'Cargo.toml').read_text('utf-8'))['workspace']['package']['version']


def esc(value):
    return html.escape(str(value), quote=True)


def icon(name, cls=''):
    return f'<svg class="{esc(cls)}" aria-hidden="true"><use href="/assets/icons.svg#{esc(name)}"/></svg>'


def safe_url(value):
    if not value:
        return ''
    if not isinstance(value, str):
        raise ValueError('Website URLs must be strings')
    parts = urlsplit(value)
    if parts.scheme != 'https' or not parts.hostname or parts.username or parts.password or any(c.isspace() for c in value):
        raise ValueError('Website and download URLs must be absolute HTTPS URLs without credentials')
    return value


def configuration(config):
    result = dict(config)
    base_path = result.get('base_path', '')
    if not isinstance(base_path, str):
        raise ValueError('base_path must be a URL path')
    base_path = base_path.rstrip('/')
    if base_path and (not re.fullmatch(r'(?:/[A-Za-z0-9_.-]+)+', base_path)
                      or any(part in ('.', '..') for part in base_path.split('/'))):
        raise ValueError('base_path must be a local URL path without traversal, queries or fragments')
    result['base_path'] = base_path
    result['site_url'] = safe_url(result.get('site_url', '')).rstrip('/')
    if result['site_url']:
        parts = urlsplit(result['site_url'])
        if parts.path or parts.query or parts.fragment or parts.port:
            raise ValueError('site_url must be an HTTPS origin without a path or port')
    result['repository'] = safe_url(result.get('repository', ''))
    release = result.get('release') or {}
    version = release.get('version', '')
    assets = release.get('assets') or {}
    if assets and not version:
        raise ValueError('Download assets require a published release version')
    if not isinstance(version, str) or any(c in version for c in '\r\n<>'):
        raise ValueError('Invalid release version')
    for system, asset in assets.items():
        if system not in ('windows', 'linux', 'macos'):
            raise ValueError(f'Unknown release platform: {system}')
        if not isinstance(asset, dict) or not asset.get('url'):
            raise ValueError('Each release asset needs a URL')
        safe_url(asset['url'])
    result['release'] = {'version': version, 'assets': assets}
    return result


def prefix_urls(text, base_path):
    """Mount generated HTML, CSS and script URL literals below a project path.

    Source templates use root-relative URLs. Leave external, protocol-relative,
    data and fragment URLs intact; localization runs after HTML URLs are mounted.
    """
    if not base_path:
        return text
    text = re.sub(r'''(["'])/(?!/)''', lambda match: match[1] + base_path + '/', text)
    return re.sub(r'(url=)/(?!/)', lambda match: match[1] + base_path + '/', text)


def faq_html(items=FAQ):
    return '<div class="faq-list">' + ''.join(
        f'<details><summary>{esc(q)}{icon("plus")}</summary><p>{esc(a)}</p></details>' for q, a in items
    ) + '</div>'


def feature_cards(items=FEATURES):
    return '<div class="feature-grid">' + ''.join(
        f'<a class="feature-card" href="/features/{slug}.html"><span class="feature-icon">{icon(mark)}</span>'
        f'<h3>{esc(title)}</h3><p>{esc(summary)}</p><span class="text-link">Explore feature {icon("arrow")}</span></a>'
        for slug, title, summary, mark, detail in items
    ) + '</div>'


def downloads(config):
    release = config['release']
    available = bool(release['assets'])
    if available:
        notice = f'<div class="release-notice"><span class="status-dot"></span><div><strong>Published release {esc(release["version"])}</strong><p>Choose an available package below.</p></div></div>'
    else:
        notice = '<div class="release-notice"><span class="status-dot"></span><div><strong>Public downloads are not available yet.</strong><p>Swoopdl is under development. Official packages will appear here when they are published.</p></div></div>'
    rows = []
    systems = [('windows', 'Windows', 'Windows 10 & 11', 'windows'), ('linux', 'Linux', 'Desktop & server', 'terminal'), ('macos', 'macOS', 'Experimental support', 'apple')]
    for system, name, sub, mark in systems:
        asset = release['assets'].get(system)
        action = (f'<a class="button" href="{esc(asset["url"])}">{icon("download")}{esc(asset.get("label", "Download for " + name))}</a>'
                  if asset else f'<p class="package-unavailable">No public package is available for {name} yet.</p><span class="availability-pill">Not published yet</span>')
        rows.append(f'<article class="platform-card" id="{system}"><span class="platform-icon">{icon(mark)}</span><h2>{name}</h2><p class="platform-sub">{sub}</p>{action}</article>')
    return notice + '<div class="platform-grid">' + ''.join(rows) + '</div>'


def article(title, intro, blocks, label='Help', extra=''):
    body = ''.join(f'<section id="section-{index}"><h2>{esc(heading)}</h2><p>{esc(copy)}</p></section>' for index, (heading, copy) in enumerate(blocks))
    parent = {'Help': '/help/', 'Features': '/features/'}.get(label, '/')
    marker = next((row[3] for row in FEATURES if row[1] == title), 'shield' if label == 'Swoopdl' else 'folder')
    toc = ''.join(f'<a href="#section-{index}">{esc(heading)}</a>' for index, (heading, copy) in enumerate(blocks))
    return (f'<section class="page-hero container article-hero"><div><a class="back-link" href="{parent}">{icon("arrow")} {esc(label)}</a>'
            f'<h1>{esc(title)}</h1><p class="lead">{esc(intro)}</p></div>'
            f'<div class="topic-art" aria-hidden="true"><span class="topic-orbit orbit-one"></span><span class="topic-orbit orbit-two"></span><span class="topic-symbol">{icon(marker)}</span><i></i><i></i></div></section>'
            f'<div class="container article-layout"><aside class="article-sidebar"><span class="section-kicker">On this page</span><nav aria-label="On this page">{toc}</nav><a class="sidebar-help" href="/help/">{icon("browser")} Help center {icon("arrow")}</a></aside>'
            f'<div class="article-body">{body}{extra}<aside class="article-next"><h2>Keep exploring.</h2><p>Find your next step in the Swoopdl help center.</p><a class="text-link" href="/help/">Browse help {icon("arrow")}</a></aside></div></div>')


def metadata(title, description, path, config):
    bits = [f'<title>{esc(title)} — Swoopdl</title>', f'<meta name="description" content="{esc(description)}">',
            f'<meta property="og:title" content="{esc(title)} — Swoopdl">', f'<meta property="og:description" content="{esc(description)}">',
            '<meta property="og:type" content="website"><meta property="og:site_name" content="Swoopdl">']
    if config['site_url']:
        url = config['site_url'] + config['base_path'] + path
        bits.extend([f'<link rel="canonical" href="{esc(url)}">', f'<meta property="og:url" content="{esc(url)}">',
                     f'<meta property="og:image" content="{esc(config["site_url"] + config["base_path"])}/assets/social.png">',
                     '<meta property="og:image:width" content="1200"><meta property="og:image:height" content="630">',
                     '<meta property="og:image:alt" content="Swoopdl. Big downloads. Small effort.">',
                     '<meta name="twitter:card" content="summary_large_image">'])
    return '\n  '.join(bits)


def shell(body):
    return '''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
  [[META]]
  <meta name="color-scheme" content="light dark"><meta name="theme-color" content="#F8FAFC">
  <link rel="icon" href="/assets/swoopdl.png" type="image/png"><link rel="apple-touch-icon" href="/assets/swoopdl.png">
  <link rel="manifest" href="/site.webmanifest"><link rel="stylesheet" href="/styles.css">[[THEME]]<script src="/main.js" defer></script>
</head><body>[[HEADER]]<main id="main">''' + body + '</main>[[FOOTER]]</body></html>'


THEME = '''<script>(function(){try{var t=localStorage.getItem('swoopdl-theme');if(t==='light'||t==='dark')document.documentElement.dataset.theme=t;}catch(e){}})();</script>'''


def build(output=OUT, config=None, *, base_path=None):
    output = Path(output).resolve()
    # Never clear an arbitrary directory: only this builder's output or an empty destination.
    if output == ROOT.resolve() or output == WEB.resolve() or output in ROOT.resolve().parents:
        raise ValueError('Refusing to build into a source directory')
    if config is None:
        config = json.loads((WEB / 'site.json').read_text('utf-8-sig'))
    if base_path is not None:
        config = {**config, 'base_path': base_path}
    config = configuration(config)
    if output.exists():
        if output != OUT.resolve() and any(output.iterdir()) and not (output / '.swoopdl-site').exists():
            raise ValueError('Output directory is not a Swoopdl website build')
        shutil.rmtree(output)
    output.mkdir(parents=True)
    (output / '.swoopdl-site').write_text('Generated by tools/website/build.py\n', 'utf-8')
    (output / 'assets').mkdir()
    (output / 'vendor').mkdir()
    for name in ('swoopdl.png', 'icons.svg', 'social.png', 'flow-art.svg', 'download-graffiti.svg'):
        shutil.copy2(WEB / 'assets' / name, output / 'assets' / name)
    shutil.copytree(WEB / 'vendor/plus-jakarta-sans', output / 'vendor/plus-jakarta-sans')
    for name in ('styles.css', 'main.js', 'localize.js'):
        text = (WEB / name).read_text('utf-8')
        (output / name).write_text(prefix_urls(text, config['base_path']), 'utf-8')
    font_path = '/vendor/plus-jakarta-sans/plus-jakarta-sans-latin.ttf'
    def asset_url(path):
        digest = hashlib.sha256((output / path.lstrip('/')).read_bytes()).hexdigest()[:10]
        return path + '?v=' + digest
    font_url = asset_url(font_path)
    stylesheet = output / 'styles.css'
    stylesheet.write_text(stylesheet.read_text('utf-8').replace(font_path, font_url), 'utf-8')
    versioned_assets = {path: asset_url(path) for path in ('/styles.css', '/main.js', '/localize.js')}
    versioned_assets[font_path] = font_url
    shutil.copy2(ROOT / 'LICENSE', output / 'LICENSE.txt')
    shutil.copy2(WEB / 'content/third-party.tsv', output / 'third-party.tsv')
    pages = []
    published = bool(config['release']['assets'])
    published_version = esc(config['release']['version'])
    package_help = ('Choose an available package on the download page.' if published else 'Public packages are not available yet.')
    feature_note = 'These features describe the current development source. Availability can differ between builds.'
    if not published:
        feature_note += ' Public packages are not published yet.'
    download_answer = (f'Release {config["release"]["version"]} is available. Visit the download page for published packages and setup instructions.' if published else 'Public downloads are not available yet. Official packages will appear on the download page when they are published.')
    download_answer += (' The source repository is linked in the footer.' if config['repository'] else ' There is no connected public repository on this website yet.')
    header = (WEB / 'partials/header.html').read_text('utf-8')
    footer = (WEB / 'partials/footer.html').read_text('utf-8')
    if config['repository']:
        footer = footer.replace('</div><div class="footer-bottom">', f'<a class="text-link" href="{esc(config["repository"])}">Source repository {icon("arrow")}</a></div><div class="footer-bottom">')

    def write(path, title, description, body=None, template=None):
        text = (WEB / template).read_text('utf-8') if template else shell(body)
        kind = 'home' if path == '/' else 'help' if path.startswith('/help') else 'features' if path.startswith('/features') else 'inner'
        text = text.replace('<body>', f'<body class="page-{kind}">')
        text = text.replace('</head>', '<script src="/localize.js" defer></script></head>')
        replacements = {'META': metadata(title, description, path, config), 'HEADER': header, 'FOOTER': footer,
                        'THEME': THEME, 'VERSION': esc(SOURCE_VERSION), 'DOWNLOADS': downloads(config),
                        'FAQ': faq_html(), 'FEATURES': feature_cards(FEATURES[:9]),
                        'RELEASE_NOTE': ('Published release ' + published_version + '. Choose an available package.' if published else 'In development. Public downloads coming later.'),
                        'CLOSING_TITLE': ('Your next file.<br>Right this way.' if published else 'Good things.<br>Coming down the line.'),
                        'CLOSING_COPY': ('Choose your platform and get started with Swoopdl.' if published else 'Swoopdl is taking shape. Check availability and get ready.'),
                        'DOWNLOAD_ANSWER': esc(download_answer), 'PACKAGE_HELP': esc(package_help)}
        for key, value in replacements.items():
            text = text.replace('[[' + key + ']]', value)
        for asset_path, versioned in versioned_assets.items():
            text = text.replace('"' + asset_path + '"', '"' + versioned + '"')
        target = output / (path.lstrip('/') + ('index.html' if path.endswith('/') else ''))
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, 'utf-8', newline='\n')
        if path != '/404.html':
            pages.append(path)

    write('/', 'Downloads, with a little more control', 'Swoopdl is a free, open-source desktop download manager for files, torrents, and video links.', template='index.html')
    write('/download.html', 'Get Swoopdl', 'Check Swoopdl package availability and learn how to set up the application and browser integration.', template='download.html')
    write('/404.html', 'Page not found', 'Find your way back to Swoopdl.', template='404.html')
    write('/features/', 'A place for every download', 'Explore Swoopdl download, organization, connection, and desktop features.',
          '<section class="page-hero container"><span class="section-kicker">Made to keep things moving</span><h1>A place for<br>every download.</h1><p class="lead">The tools you need, together in one desktop app.</p><p class="development-note">' + esc(feature_note) + '</p></section><section class="container section-bottom">' + feature_cards() + '</section>')
    for slug, title, summary, mark, detail in FEATURES:
        write(f'/features/{slug}.html', title, summary,
              article(title, summary, [('How it works', detail), ('Get started', 'Visit the download page for package availability and setup information.')], label='Features',
                      extra='<a class="button" href="/download.html">Get Swoopdl ' + icon('download') + '</a>'))
    help_cards = '<div class="help-grid">' + ''.join(f'<a class="help-card" href="/help/{slug}.html"><h2>{esc(title)}</h2><p>{esc(intro)}</p>{icon("arrow")}</a>' for slug, (title, intro, blocks) in HELP.items()) + '</div>'
    write('/help/', 'A hand when you need it', 'Get started with Swoopdl, set up browser integration, and find download help.', '<section class="page-hero container"><span class="section-kicker">Swoopdl help</span><h1>A hand when<br>you need it.</h1><p class="lead">Start here. Get set up. Keep your downloads moving.</p></section><section class="container section-bottom">' + help_cards + '</section>')
    for slug, (title, intro, blocks) in HELP.items():
        write(f'/help/{slug}.html', title, intro, article(title, intro, blocks))
    write('/faq/', 'Good questions. Straight answers.', 'Answers about Swoopdl pricing, platform support, browser integration, and downloads.', '<section class="page-hero container narrow"><h1>Good questions.<br>Straight answers.</h1><p class="lead">A few things you might want to know.</p></section><section class="container narrow section-bottom">' + faq_html() + '</section>')
    for slug, (title, label, blocks) in LEGAL.items():
        write(f'/{slug}.html', label, title, article(title, label, blocks, label='Swoopdl'))
    license_extra = '<p><a class="text-link" href="/LICENSE.txt">Read the complete software license</a></p><p><a class="text-link" href="/third-party.tsv">Third-party software and license list</a></p><p><a class="text-link" href="/vendor/plus-jakarta-sans/OFL.txt">Plus Jakarta Sans font license</a></p>'
    write('/license.html', 'Open-source licenses', 'Swoopdl software and website license information.', article('Free software. Shared foundations.', 'Open-source licenses', [
        ('Swoopdl', 'The application is licensed under the GNU General Public License, version 3 or later. Existing copyright notices and third-party obligations apply. The complete license is provided below.'),
        ('Copyright and credits', 'Copyright © 2026 Ahmed Nefzaoui and contributors. Modified distributions retain existing authorship and license notices. The approved Swoopdl visual identity is provided in the source checkout.'),
        ('This website', 'The website uses Plus Jakarta Sans by Tokotype under the SIL Open Font License 1.1. Fonts are self-hosted. The interface illustration is an original website concept, not a screenshot of the shipping application.')], label='Swoopdl', extra=license_extra))
    extra_pages(write, article, icon, esc, ROOT, SOURCE_VERSION)

    # Keep useful old routes working without publishing old copy or screenshots.
    aliases = {'/benchmark.html': '/features/acceleration.html', '/idm-alternative.html': '/features/',
               '/features/import-idm.html': '/features/', '/faq.html': '/faq/'}
    for doc in (ROOT / 'docs').glob('*.md'):
        route = '/help/' + doc.stem + '.html'
        if route not in pages:
            aliases[route] = '/help/'
    for path, destination in aliases.items():
        write(path, 'Find your next step', 'This page has moved within the Swoopdl website.', '<section class="container narrow page-hero"><h1>Find your next step.</h1><p class="lead">This page has a new home.</p><a class="button" href="' + destination + '">Continue ' + icon('arrow') + '</a></section>')
        target = output / path.lstrip('/')
        text = target.read_text('utf-8').replace('<meta name="color-scheme"', '<meta name="robots" content="noindex"><meta http-equiv="refresh" content="0;url=' + destination + '"><meta name="color-scheme"')
        target.write_text(text, 'utf-8')
    (output / 'site.webmanifest').write_text(json.dumps({'name': 'Swoopdl', 'short_name': 'Swoopdl', 'start_url': config['base_path'] + '/', 'display': 'browser', 'background_color': '#F8FAFC', 'theme_color': '#2563EB', 'icons': [{'src': config['base_path'] + '/assets/swoopdl.png', 'sizes': '1024x1024', 'type': 'image/png'}]}, indent=2), 'utf-8')
    robots = 'User-agent: *\nAllow: /\n'
    if config['site_url']:
        (output / 'CNAME').write_text(urlsplit(config['site_url']).hostname + '\n', 'utf-8')
        sitemap = ET.Element('urlset', xmlns='http://www.sitemaps.org/schemas/sitemap/0.9')
        for path in pages:
            if path in aliases:
                continue
            ET.SubElement(ET.SubElement(sitemap, 'url'), 'loc').text = config['site_url'] + config['base_path'] + path
        ET.ElementTree(sitemap).write(output / 'sitemap.xml', encoding='utf-8', xml_declaration=True)
        robots += '\nSitemap: ' + config['site_url'] + config['base_path'] + '/sitemap.xml\n'
    (output / 'robots.txt').write_text(robots, 'utf-8')
    (output / '.nojekyll').write_text('', 'utf-8')
    (output / 'llms.txt').write_text(('# Swoopdl\n\nFree, open-source desktop download manager. ' + download_answer + '\n\n- Features: /features/\n- Availability and setup: /download.html\n- Help: /help/\n- License: /license.html\n').replace(': /', ': ' + config['base_path'] + '/'), 'utf-8')
    # Rewrite before collecting translation marks so localized links keep the same mount.
    for page in output.rglob('*.html'):
        page.write_text(prefix_urls(page.read_text('utf-8'), config['base_path']), 'utf-8')
    localization.publish(output)
    print(f'Built {len(pages)} pages offline -> {output}')
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--serve', action='store_true', help='Serve the generated website locally')
    parser.add_argument('--port', type=int, default=8765)
    parser.add_argument('--base-path', help='URL prefix for project hosting, for example /swoopdl')
    args = parser.parse_args()
    config = json.loads((WEB / 'site.json').read_text('utf-8-sig'))
    if args.base_path is not None:
        config['base_path'] = args.base_path
    config = configuration(config)
    build(config=config)
    if args.serve:
        handler = functools.partial(PreviewHandler, directory=str(OUT), base_path=config['base_path'])
        print(f'Preview: http://127.0.0.1:{args.port}{config["base_path"]}/', flush=True)
        http.server.ThreadingHTTPServer(('127.0.0.1', args.port), handler).serve_forever()


class PreviewHandler(http.server.SimpleHTTPRequestHandler):
    """Always show current build output instead of an older browser-cached page."""

    def __init__(self, *args, base_path='', **kwargs):
        self.base_path = base_path
        super().__init__(*args, **kwargs)

    def send_head(self):
        if self.base_path:
            path = urlsplit(self.path).path
            if path == self.base_path:
                self.send_response(301)
                self.send_header('Location', self.base_path + '/')
                self.end_headers()
                return None
            if not path.startswith(self.base_path + '/'):
                self.send_error(404)
                return None
        return super().send_head()

    def translate_path(self, path):
        return super().translate_path(path[len(self.base_path):])

    def end_headers(self):
        self.send_header('Cache-Control', 'no-store')
        super().end_headers()


if __name__ == '__main__':
    main()
