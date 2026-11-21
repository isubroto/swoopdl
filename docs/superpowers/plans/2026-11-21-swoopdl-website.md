# Swoopdl Website Implementation Plan

> **For agentic workers:** Use executing-plans to implement this plan task-by-task.

**Goal:** Apply the approved Swoopdl design and run a verified local preview.

**Architecture:** Buildless HTML/CSS/JavaScript, with a small standard-library Python builder for shared layout, curated content pages, optional release configuration, and clean static assets. No runtime service or GitHub dependency.

**Tech Stack:** HTML, CSS, JavaScript, Python 3, browser QA through Playwright.

**Spec:** `docs/superpowers/specs/2026-11-21-swoopdl-website-design.md`

## Global Constraints

- Use glacier white, slate, cobalt, teal, self-hosted Plus Jakarta Sans, and approved Swoopdl artwork.
- Do not publish inherited branding, screenshots, analytics, benchmarks, upstream links, or stale translations.
- No configured release means an honest unavailable state, not a fabricated download.
- Preserve required legal notices and keep changes local; do not commit or deploy.
- Work in place: Git has no initial commit and the entire checkout is untracked. Save a local backup before replacement.

## Review Focus

- An offline build must neither request upstream data nor require missing third-party Python modules.
- Missing production URLs must omit canonical links and CNAME without breaking local navigation.
- Release configuration must reject unsafe URLs and never confuse source and published versions.
- Theme, navigation, tabs, FAQs, and layout must work at narrow widths and with a keyboard.
- Generated pages must not reintroduce old branding through copied assets or inherited content.

### Task 1: Clean build and release behavior

- [x] Back up website and old builder under ignored `target/`.
- [x] Write meaningful integration tests for offline build, no inherited content/assets, local-link integrity, no-release state, and configured release validation.
- [x] Run tests and observe failures against current behavior.
- [x] Replace builder with an offline, configuration-driven standard-library implementation.
- [x] Curate feature/help/legal content and preserve useful route aliases.
- [x] Run integration tests and inspect actual output.

### Task 2: Original website design

- [x] Replace homepage, download page, 404, styles, and interaction script.
- [x] Add approved logo exports, local font files, and original HTML interface illustration.
- [x] Style generated pages consistently with the new site.
- [x] Verify JavaScript syntax and output/link checks.

### Task 3: Browser verification and running preview

- [x] Run local preview on `http://127.0.0.1:8765/`.
- [x] Exercise theme persistence, mobile navigation, tabs, FAQ, unavailable downloads, and keyboard behavior.
- [x] Inspect mobile/desktop screenshots in both themes; check 320, 390, 768, 1024, and 1440px widths.
- [x] Request one independent final review, resolve important issues, and rerun affected checks.
- [x] Leave preview running and report URL and verification results.

## Execution ledger

- Approved by user: apply this design and run the website.
- Ruling: use direct source edits and an ignored ZIP backup because this repository has no HEAD. Do not create an initial commit.
- Ruling: use Python's standard library to remove the missing Markdown dependency and guarantee offline builds. Existing legacy documents are not copied wholesale; publish curated content and route aliases.

- Task 1 complete: standard-library offline builder; integration checks verify release configuration, clean assets, links, and unavailable-package behavior.
- Task 2 complete: original responsive light/dark design, approved Swoopdl artwork, variable Plus Jakarta Sans, and social image. Retired content is archived outside the publish directory.
- Task 3 complete: nine integration tests pass; JavaScript syntax passes; 14 browser interaction checks and 60 responsive/theme cases pass. No JavaScript errors observed.
- Review: fixed inconsistent configured-release copy and missing social metadata. Added cache-busted assets and no-store preview responses after reproducing stale browser content.
- Preview remains running at http://127.0.0.1:8765/. The process ID is recorded in target/website-preview.pid.
