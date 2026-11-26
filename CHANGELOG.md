# Swoopdl Changelog

This log records Swoopdl application development releases. Version numbers follow the rule that `0.x.0` is a feature release and `0.x.x` (nonzero final component) is a fix release. A release candidate is not considered Windows-qualified until its Windows tests pass.

## Unreleased — Site Grabber file exploration

### Added

- Separate Files and Offline website workflows, with bounded metadata-only exploration, durable checked candidates, source-folder results, and selected-file queue handoff.
- Guarded normal-engine jobs with isolated persistence, duplicate dispatch recovery, fresh robots checks, checked DNS/redirects/routing, and origin-scoped session context.
- Flat, original-subfolder, and category layouts with safe names and non-destructive collisions; stopped-project editing and distinct schedule actions.
- Explicit import of browser-rendered links, in-memory browser sessions, and HLS/DASH handoff to existing media tools.

### Fixed

- Classify linked media before applying page depth or file filters; detect HTML disguised as video and cap ignored-Range metadata responses.
- Include nested native UI modules when extracting translatable English strings.

## [0.1.7] — Site Grabber recovery and routing (release candidate)

### Fixed

- Accept verified DIRECT routes selected through system proxy settings and recheck routing on every Site Grabber redirect.
- Resolve Windows automatic discovery instead of rejecting its enabled flag; retain configured proxy fallback when no WPAD file exists.
- Keep native proxy discovery off the async thread and enforce the crawl deadline so pause and cancellation stay responsive.
- Distinguish robots lookup failures from real exclusions, retain failed URLs for retry, and preserve actionable sanitized causes in capture events and reports.
- Create a dedicated project subfolder by default when selecting a base folder, including drive roots.
- Show the final capture path and check destination conflicts before leaving the save-location step and before saving metadata.
- Let projects with no captured data change destination without losing their capture settings.
- Show failed starts and capture failures as Failed, retain their cause after reopening, and suspend schedules until manual retry.
- Show template-specific file guidance and retain explicit per-file and run budgets for large videos.

### Changed
- Set all workspace packages and application version resources to 0.1.7.

## [0.1.6] — Site Grabber projects and setup wizard (release candidate)

### Added
- Added **Tasks → Site Grabber** for saved static HTML/CSS website captures, while retaining Link Grabber for individual download links.
- Added scoped crawling, robots rules, depth and URL ceilings, decoded-byte budgets, retries, and durable pause/resume.
- Added offline copies with rewritten local links, immutable raw bodies, separate published generations, and sanitized capture reports.
- Added conditional synchronization, application-open schedules, and optional memory-only or verified system-vault credentials.
- Added a themed four-step setup wizard with image, audio, video, document, and offline-site templates, folder browsing, exploration rules, and file filters.

### Fixed
- Enforced checked DNS and redirect policies, origin-scoped credentials, safe output paths, and decoded-body accounting.
- Disabled active scripts and forms in offline copies and preserved earlier published copies during interrupted updates.

### Validation
- Recorded Windows tests, offline browser checks, bounded-frontier measurements, and remaining platform acceptance in [Site Grabber verification](docs/site-grabber-verification.md).
- JavaScript rendering, app-closed scheduling, and proxy-based captures remain unsupported. Destination validation and recovery improvements are recorded under **0.1.7**.

## [0.1.5] — Fluent download-dialog refinement (release candidate)

### Fixed
- Separated progress-window navigation tabs from the download status content, progress meter, and action row to prevent overlapping controls.
- Improved Add URL, Download Complete, and Download File Info alignment, visual hierarchy, field sizes, and footer spacing.
- Preserved Classic layout geometry and existing download actions, keyboard shortcuts, and state handling.

### Changed
- Improved Fluent-only dialog headers, icon tiles, typography, and button emphasis.
- Added layout geometry checks and a guarded, reversible source upgrade.

## [0.1.4] — Fluent UX unification

- Introduced a consistent Fluent dialog presentation and typography hierarchy.
- Updated Add URL, Download Complete, File Info, and download-progress presentation.
- Coordinated presentation shells for torrent, video, batch, link-grabber, RSS, and speed-limiter dialogs.

## [0.1.3] — Fluent Settings reflow

- Expanded and aligned controls across Configuration pages while retaining their settings behavior.
- Improved navigation, content spacing, and form readability.

## [0.1.2] — Fluent component consistency

- Introduced consistent Settings surfaces, spacing, and controls.
- Updated download-related Settings and keyboard interaction.

## [0.1.1] — Theme and installer fixes

- Refined Swoopdl Fluent and Aqua presentation and improved menu spacing.
- Made Fluent the default for new profiles and displayed the classic theme as **IDM Classic**.
- Removed obsolete choices from the built-in theme selector while keeping compatibility with existing user themes.
- Corrected installation and upgrade validation issues.

## [0.1.0] — Initial release

- Initial Swoopdl desktop download manager release.
- Download management, categories, queues, scheduling, progress tracking, and file actions.
- Torrent, video, browser-integration, and configurable download options.
- Cross-platform Rust desktop application foundation.
