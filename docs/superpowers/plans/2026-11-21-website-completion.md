# Website consistency, artwork, and localization

Continue the approved Swoopdl design. The user's correction explicitly requests every page, decorative background artwork, and multilingual support like the application.

- [x] Replace useful route placeholders with real themed pages, including themes, languages, development status, contribution information, and help guides.
- [x] Apply shared original blue/teal background artwork and richer page layouts to all page families. Preserve readability in both themes and at small widths.
- [x] Use `i18n/languages.tsv` as the language source of truth. Add a searchable, accessible picker that preserves the current page and remembers the chosen language.
- [x] Build translated pages from checked catalogs; retain local assets and offline builds. Add RTL layout support for Arabic-script and Hebrew locales.
- [x] Localize runtime strings as well as static content. Mark untranslated English passages honestly and report translation coverage; do not claim incomplete locales are fully translated.
- [x] Verify page completeness, locale links, scripts, mobile/RTL layout, theme behavior, and language switching. Rebuild and leave the preview running.

Implementation decisions: reuse the existing HTML segment parser and app language catalogs. Keep the original Swoopdl identity and avoid any inherited branding. Generate artwork as original SVG. Machine translations must preserve placeholders, source names, and links; source legal notices remain intact.

## Verification

- All 12 build/integration checks pass. JavaScript syntax checks pass.
- Browser regressions pass, including language persistence, picker search, keyboard/escape/focus, English restoration, mixed-language annotation, runtime preview metadata, mobile menu, and theme persistence.
- 384 layout cases pass across eight page families, six languages, light/dark themes, and widths 320/390/768/1440px.
- All 86 app locales are represented. Bengali, Arabic, and French each cover all 629 current source strings. Other catalogs currently provide partial UI translations with explicit English fallback. No external translation service is used by the build or website.
- Useful theme, language, status, contribution, credit, and help routes now render dedicated pages. Shared SVG background artwork appears on every page family.
- Review findings fixed: partial English fragments now carry their own language metadata; runtime preview language attributes update together with their text.
- Preview remains available at http://127.0.0.1:8765/.
