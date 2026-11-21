"""Integration checks for the public website and its release configuration."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from html.parser import HTMLParser
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[2]


class References(HTMLParser):
    def __init__(self):
        super().__init__()
        self.urls = []

    def handle_starttag(self, tag, attrs):
        self.urls.extend(value for key, value in attrs if key in ('src', 'href') and value)


class WebsiteBuildTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        spec = importlib.util.spec_from_file_location('website_builder', ROOT / 'tools/website/build.py')
        cls.builder = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.builder)

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output = Path(self.temp.name) / 'site'

    def test_default_build_is_offline_and_does_not_advertise_unpublished_releases(self):
        self.builder.build(self.output)
        home = (self.output / 'index.html').read_text('utf-8')
        download = (self.output / 'download.html').read_text('utf-8')
        self.assertIn('Big downloads.', home)
        self.assertIn('Public downloads are not available yet', download)
        self.assertNotIn('rel="canonical"', home)
        self.assertNotIn('downloadUrl', home)
        self.assertFalse((self.output / 'CNAME').exists())

    def test_generated_pages_have_no_inherited_branding_or_external_tracking(self):
        self.builder.build(self.output)
        forbidden = ('mister download', 'misterdownload', 'master download', 'mdm', 'statcounter',
                     'anefzaoui', 'ejoleppdfgjdndddcjoehopknmlglojp', '5.4×', 'caa200c')
        for file in self.output.rglob('*'):
            if file.suffix not in ('.html', '.css', '.js', '.json', '.xml', '.txt', '.svg'):
                continue
            text = file.read_text('utf-8').lower()
            for value in forbidden:
                self.assertNotIn(value, text, str(file))
        self.assertFalse((self.output / 'assets/screens').exists())

    def test_all_internal_links_and_images_exist(self):
        self.builder.build(self.output)
        for page in self.output.rglob('*.html'):
            parser = References()
            parser.feed(page.read_text('utf-8'))
            for url in parser.urls:
                parts = urlsplit(url)
                if parts.scheme or parts.netloc or not parts.path:
                    continue
                path = (self.output / unquote(parts.path).lstrip('/')) if parts.path.startswith('/') else page.parent / unquote(parts.path)
                if parts.path.endswith('/'):
                    path = path / 'index.html'
                self.assertTrue(path.exists(), f'{page.relative_to(self.output)}: {url}')

    def test_configured_release_and_site_metadata_use_only_configured_values(self):
        config = {'site_url': 'https://downloads.example.test', 'repository': '',
                  'release': {'version': '1.2.3', 'assets': {
                      'windows': {'url': 'https://downloads.example.test/Swoopdl-setup.exe', 'label': 'Windows installer'}
                  }}}
        self.builder.build(self.output, config)
        home = (self.output / 'index.html').read_text('utf-8')
        download = (self.output / 'download.html').read_text('utf-8')
        self.assertIn('https://downloads.example.test/', home)
        self.assertIn('https://downloads.example.test/Swoopdl-setup.exe', download)
        self.assertIn('1.2.3', download)
        self.assertIn('No public package is available for Linux yet', download)
        self.assertEqual((self.output / 'CNAME').read_text().strip(), 'downloads.example.test')

    def test_release_status_is_consistent_across_pages_and_machine_readable_copy(self):
        config = {'repository': 'https://example.test/project', 'release': {
            'version': '1.2.3', 'assets': {'windows': {'url': 'https://example.test/setup.exe'}}}}
        self.builder.build(self.output, config)
        for name in ('index.html', 'faq/index.html', 'features/index.html',
                     'features/acceleration.html', 'help/getting-started.html', 'llms.txt'):
            text = (self.output / name).read_text('utf-8')
            self.assertNotIn('Public downloads coming later', text, name)
            self.assertNotIn('Public packages are not published', text, name)
            self.assertNotIn('no connected public repository', text, name)
            self.assertNotIn('Public packages are not available yet', text, name)
        self.assertIn('1.2.3', (self.output / 'index.html').read_text('utf-8'))

    def test_configured_site_has_a_local_social_preview(self):
        self.builder.build(self.output, {'site_url': 'https://example.test'})
        text = (self.output / 'index.html').read_text('utf-8')
        self.assertIn('property="og:image" content="https://example.test/assets/social.png"', text)
        self.assertTrue((self.output / 'assets/social.png').is_file())

    def test_unsafe_release_url_is_rejected(self):
        config = {'release': {'version': '1.0', 'assets': {'windows': {
            'url': 'javascript:alert(1)', 'label': 'Installer'}}}}
        with self.assertRaises(ValueError):
            self.builder.build(self.output, config)

    def test_stale_files_are_removed_on_rebuild(self):
        self.builder.build(self.output)
        (self.output / 'obsolete.html').write_text('Old content')
        self.builder.build(self.output)
        self.assertFalse((self.output / 'obsolete.html').exists())

    def test_assets_are_versioned_and_templates_are_fully_rendered(self):
        self.builder.build(self.output)
        home = (self.output / 'index.html').read_text('utf-8')
        self.assertTrue('/styles.css?v=' in home)
        self.assertTrue('/main.js?v=' in home)
        for page in self.output.rglob('*.html'):
            self.assertTrue('[[' not in page.read_text('utf-8'), str(page))

    def test_all_page_families_have_real_content_and_shared_artwork(self):
        self.builder.build(self.output)
        for name in ('themes.html', 'languages.html', 'news.html', 'contributing.html',
                     'help/guide-first-download.html', 'features/torrents.html'):
            text = (self.output / name).read_text('utf-8')
            self.assertTrue('http-equiv="refresh"' not in text, name)
            self.assertTrue('page-translations' in text, name)
            self.assertTrue('data-language-open' in text, name)
        self.assertTrue((self.output / 'assets/flow-art.svg').is_file())

    def test_language_manifest_matches_app_and_includes_rtl_locales(self):
        import json
        self.builder.build(self.output)
        manifest = json.loads((self.output / 'locales/index.json').read_text('utf-8'))
        expected = {line.split('\t')[0] for line in (ROOT / 'i18n/languages.tsv').read_text('utf-8').splitlines() if line and not line.startswith('#')}
        self.assertEqual({item['code'] for item in manifest['languages']}, expected)
        self.assertEqual(next(item['dir'] for item in manifest['languages'] if item['code'] == 'ar'), 'rtl')
        self.assertEqual(next(item['dir'] for item in manifest['languages'] if item['code'] == 'he'), 'rtl')
        for code in expected - {'en'}:
            self.assertTrue((self.output / f'locales/{code}.json').is_file(), code)

    def test_primary_language_catalogs_cover_current_source(self):
        import json
        self.builder.build(self.output)
        manifest = json.loads((self.output / 'locales/index.json').read_text('utf-8'))
        for code in ('bn', 'ar', 'fr'):
            coverage = manifest['coverage'][code]
            self.assertGreaterEqual(coverage['translated'] / coverage['total'], .99, code)


if __name__ == '__main__':
    unittest.main()
