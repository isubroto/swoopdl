"""Offline website catalogs using the application's language list and translation data."""
from __future__ import annotations

import hashlib
import html
import json
from pathlib import Path
import re

import sitei18n

ROOT = Path(__file__).resolve().parents[2]
WEB = ROOT / 'website'
RTL = {'ar', 'fa', 'he', 'ur', 'ug'}
FORBIDDEN = re.compile(r'mister\s*download|master\s*download|\bmdm\b|statcounter|anefzaoui', re.I)


def languages():
    rows = []
    for line in (ROOT / 'i18n/languages.tsv').read_text('utf-8').splitlines():
        if not line or line.startswith('#'):
            continue
        code, english, native, *_ = line.split('\t')
        rows.append({'code': code, 'english': english, 'native': native,
                     'dir': 'rtl' if code in RTL else 'ltr'})
    return rows


def normalize(value):
    return re.sub(r'\s+', ' ', html.unescape(value).replace('&', '')).strip().rstrip('.:')


def app_catalog(code):
    path = ROOT / 'i18n' / f'{code}.tsv'
    result = {}
    if path.exists():
        for line in path.read_text('utf-8').splitlines():
            fields = line.split('\t')
            if len(fields) != 3 or fields[0].startswith(('#', '@')):
                continue
            source, translation = fields[1:]
            if translation and not FORBIDDEN.search(translation):
                result.setdefault(normalize(source), translation.replace('&', ''))
    return result


def instrument(doc):
    """Annotate text ranges without wrappers that would change grid/flex layout."""
    tokens, spans, attrs = sitei18n.walk(doc)
    raw = [token.raw for token in tokens]
    segments = []
    for start, end, segment in spans:
        if segment.where == 'page title (browser tab, search results)':
            continue
        key = str(len(segments))
        segments.append({'id': key, 'source': segment.text, 'marks': segment.marks})
        raw[start] = f'<!--swoop-text:{key}-->' + raw[start]
        raw[end - 1] += f'<!--/swoop-text:{key}-->'
    by_token = {}
    for index, name, value in attrs:
        by_token.setdefault(index, {})[name] = html.unescape(value)
    for index, values in by_token.items():
        raw[index] = re.sub(r'\s*/?>$', lambda m: ' data-l10n-attrs="' + html.escape(json.dumps(values, ensure_ascii=False), quote=True) + '"' + m.group(), raw[index])
    payload = json.dumps({'segments': segments}, ensure_ascii=False).replace('<', '\\u003c')
    return ''.join(raw).replace('</body>', f'<script type="application/json" id="page-translations">{payload}</script></body>')


def publish(output: Path):
    """Compile only translations for current source strings; builds never use a service."""
    pages = list(output.rglob('*.html'))
    wanted = {}
    runtime = json.loads((ROOT / 'tools/website/core_strings.json').read_text('utf-8'))
    for page in pages:
        for segment in sitei18n.page_segments(page.read_text('utf-8')):
            wanted[segment.text] = segment
    for text in runtime:
        wanted.setdefault(text, sitei18n.Segment(text))
    # Runtime preview copy is not present in static HTML until a feature is selected.
    script = (WEB / 'main.js').read_text('utf-8')
    for value in re.findall(r"(?:label|title|description|stat|meta|note): '([^']+)'", script):
        wanted.setdefault(value, sitei18n.Segment(value))
    core_path = WEB / 'locales/core.json'
    core = json.loads(core_path.read_text('utf-8')) if core_path.exists() else {}
    locale_dir = output / 'locales'
    locale_dir.mkdir()
    report = {}
    for language in languages():
        code = language['code']
        if code == 'en':
            continue
        app = app_catalog(code)
        old = sitei18n.Catalog(code)
        translations = {}
        for source, segment in wanted.items():
            existing = old.get(segment)
            value = existing or app.get(normalize(source))
            if value and not FORBIDDEN.search(value) and not sitei18n.check_marks(segment, value):
                translations[source] = value
        authored_path = WEB / 'locales' / f'{code}.json'
        authored = json.loads(authored_path.read_text('utf-8')).get('translations', {}) if authored_path.exists() else {}
        for source, value in {**core.get(code, {}), **authored}.items():
            if source in wanted and value and not FORBIDDEN.search(value) and not sitei18n.check_marks(wanted[source], value):
                translations[source] = value
        # Plain core terms also translate parts of a heading with inline styling.
        for source, value in core.get(code, {}).items():
            if source in runtime and value and not FORBIDDEN.search(value):
                translations[source] = value
        data = {'language': code, 'dir': language['dir'], 'translations': translations}
        (locale_dir / f'{code}.json').write_text(json.dumps(data, ensure_ascii=False, separators=(',', ':')), 'utf-8')
        report[code] = {'translated': sum(source in translations for source in wanted), 'total': len(wanted)}
    version = hashlib.sha256(''.join(p.read_text('utf-8') for p in sorted(locale_dir.glob('*.json'))).encode()).hexdigest()[:10]
    manifest = {'languages': languages(), 'version': version, 'coverage': report}
    (locale_dir / 'index.json').write_text(json.dumps(manifest, ensure_ascii=False, separators=(',', ':')), 'utf-8')
    for page in pages:
        page.write_text(instrument(page.read_text('utf-8')), 'utf-8')
    return report
