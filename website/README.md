# Swoopdl website

The website uses static HTML, CSS, and JavaScript. Its builder requires Python 3.11 or later and only the Python standard library. No GitHub connection, token, package installation, or network access is required to build it.

From the repository root:

```powershell
python tools/website/build.py --serve
```

Open **http://127.0.0.1:8765/**. Build output is written to `target/site`. To use another port, add `--port 8766`. Run the command again after editing source files to rebuild the website.

To reproduce GitHub project hosting, run `python tools/website/build.py --serve --base-path /swoopdl`
and open **http://127.0.0.1:8765/swoopdl/**. The Pages workflow supplies the repository name as
this prefix. Assets, navigation, fonts, language catalogs, and redirects all use that path.
Root hosting remains the default; an optional `base_path` in `site.json` sets the same prefix.

```powershell
python -m unittest discover -s tools/website -p test_build.py -v
node --check website/main.js
```

## Source layout

- `index.html`, `download.html`, and `404.html`: page templates.
- `partials/`: shared navigation, footer, and the social artwork source.
- `styles.css` and `main.js`: shared visual styles and interactions.
- `assets/`: approved Swoopdl artwork and original website assets.
- `vendor/plus-jakarta-sans/`: local variable font and its license.
- `site.json`: optional deployment and release configuration.
- `tools/website/content.py`: curated feature, help, FAQ, and information pages.

The interface illustrations are website concepts with sample data. They do not claim to be screenshots of a released application.

## Configure a real release

Leave `site_url`, `repository`, and `release.assets` empty until real values exist. With the default configuration, the website explains that public packages are not available. It does not make external requests or manufacture download links.

Each optional release asset is keyed by `windows`, `linux`, or `macos` and has an absolute HTTPS `url` and a `label`. A nonempty asset list requires `release.version`. `site_url`, when set, is an HTTPS origin without a path; it enables canonical URLs, social metadata, a sitemap, and CNAME output. `repository` adds a link to the source repository. Configuration is validated before existing output is replaced.

Only the builder's asset allowlist is published. Retired website content and assets are backed up locally under `target/website-redesign-backup`; those files are not shipped. Themes, languages, status, contribution information, and help guides have dedicated pages. Retired comparison URLs redirect to useful current destinations.

## Languages

The language directory and searchable picker use the application's 86-language list in `i18n/languages.tsv`. Language choice is remembered locally and preserved in page links with `?lang=<code>`. Arabic, Persian, Hebrew, Urdu, and Uyghur use right-to-left layout. Switching languages requires JavaScript; English reading and navigation remain available without it.

The builder compiles local catalogs from `website/locales/`, exact matching existing site translations, and matching application UI translations. It never calls a translation service. The runtime loads only local JSON files. Text that lacks a translation remains explicitly marked as English, with a notice on the page. Translation coverage varies by language; the generated `locales/index.json` reports exact coverage. Translations should be reviewed by native speakers before publication.

Edit `website/locales/<code>.json` using the `translations` map. Keys are exact English source segments. Preserve `<gN>`, `</gN>`, and `<xN/>` placeholders, technical names, and filenames. The build validates placeholders and excludes obsolete branding. License explanations can be translated; the complete GPL license remains available unchanged in English.

The site includes no analytics. Theme preference stays in browser local storage.
