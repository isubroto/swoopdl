# Swoopdl Changelog

This log records Swoopdl application development releases. Version numbers follow the rule that `0.x.0` is a feature release and `0.x.x` (nonzero final component) is a fix release. A release candidate is not considered Windows-qualified until its Windows tests pass.

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
