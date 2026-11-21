# Bundled Browser Extension Implementation Plan

**Goal:** Build extensions from local source and include local browser setup in Windows, Linux, and macOS packages.

**Architecture:** Extend the existing packer with one shared offline bundle. Package builders copy that bundle into their native layouts. Setup helpers register the bundled native host and guide users through browser approval using local files.

**Constraints:** No browser-store publishing or extension downloads during installation. Do not alter browser profiles or enterprise policies. Chromium uses Load unpacked; unsigned Firefox packages are temporary development add-ons. Permanent Firefox installation requires Mozilla signing. Safari is outside this Chromium/Firefox distribution change.

- [x] Add an integration test proving the bundle contains matching versions, browser assets, and setup helpers.
- [x] Run the test and confirm missing bundle failure.
- [x] Extend packing and add Unix/macOS setup helpers; preserve existing Windows helper checks.
- [x] Connect all three package builders and release/CI artifact handling.
- [x] Update installation documentation and verify bundle contents, helper behavior, shell/PowerShell syntax, and workflow YAML.

Native installer builds require their host operating systems and existing packaging prerequisites. Do not claim them tested from Windows alone.

Validation: the package checker and five offline bundle/helper integration tests pass. PowerShell and Bash syntax checks pass. All workflow YAML parses. The existing extension logic suite still fails at test.mjs:340 on the local Node 26 runtime. Full package builds remain unverified; ATTRIBUTION.md and Inno Setup are absent locally, and Linux/macOS package builds require native runners.
