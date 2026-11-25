# Site Grabber wizard UI

Design: replace the single scrolling settings form with IDM-inspired progressive disclosure, using Swoopdl's native dialog frame, palette, typography and focus behavior. Four steps: Project & start page (with working presets), Save location (asynchronous folder picker), Explore rules (depth, seed directory, exact origins, advanced prefixes), File filters (extensions, MIME, maximum resource size, run limits and review). Keep optional credentials and schedules in expandable sections. Use safe existing output paths; do not offer unsupported original-directory, browser-login or scan-only controls. Preserve existing capture, scheduling and vault behavior.

Implementation plan:
1. Add failing model tests for step validation/back navigation, image presets and resource size mapping.
2. Add wizard step/template state and scoped validation. Add async folder-picker messages using the existing runtime.
3. Replace presentation in a dedicated view module: themed native window, four-step rail, aligned fields, fixed navigation footer, and project list/details with clear status and progress.
4. Run focused UI tests, formatter, translation extractor and workspace Clippy sequentially with CARGO_BUILD_JOBS=1. Build and inspect native screenshots where the available harness permits. Record exact evidence and limitations.

Scope: UI workflow and mapping to existing capture settings only. No output-layout migration, arbitrary-subdomain permissions, JavaScript rendering or new credential mechanism. Preserve pre-existing dirty edits. No delegation, commits, pushes or releases.

Verification complete: 337 app tests passed, 1 ignored; 10 Site Grabber tests passed. Native binary build, app/all-target Clippy with warnings denied, formatting, translation extraction and whitespace checks passed. Captured and visually inspected 10 native previews (workspace plus four stages, Fluent light/dark); accessibility controls stayed within each window. Native button-click smoke was blocked by automatic approval review and was not executed. See docs/site-grabber-verification.md. Shared build-cache loss required an isolated rebuild; artifacts remain under .superpowers/sdd/2026-11-25-site-grabber-ui/. No commits or releases.
