# Site Grabber verification

The implementation is present on `feat/full-site-grabber`. Cross-platform acceptance is incomplete; this document does not certify a release.

Implemented scope includes saved projects, scoped static HTML/CSS crawling, robots policy, bounded decoded body accounting, durable pause/resume, offline generations, reports, conditional synchronization, calendar schedules, and optional memory-only or system-vault credentials. The native entry point is **Tasks → Site Grabber**.

## Automated checks

Current-source Windows checks use Rust 1.99.1 with `CARGO_BUILD_JOBS=1`.

- Grabber suites: 44 passed in the final workspace run, including transport, recovery, offline rewriting, synchronization, metadata bounds, cancelled publication, and bounded future size. The earlier standalone grabber run passed 43 tests before the future-size regression was added.
- `cargo test -p sdl-engine --lib decode_budget_tests --locked`: 4 passed. These prove reservations precede decoded reads, parallel readers cannot overread the shared budget, cancellation refunds known unused reservations, and the public transport future stays bounded for small native stacks.
- `cargo test -p sdl-app --bin swoopdl site_grabber --locked -- --test-threads=1`: 8 passed.
- `cargo fmt --all --check`, `python tools/i18n/extract.py --check`, and `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked -- --test-threads=1`: **909 passed, 22 ignored, 0 failed across 71 targets**, after the final stack fix. This includes the native app, ordinary downloads, browser integration, tools, and torrent regressions.

A fresh Windows example exposed a main-thread stack overflow after the first workspace pass. The capture and transport futures occupied 74,480 and 69,176 bytes respectively. Heap-pinning the nested workers reduced their public future sizes below 32 KiB; both size regressions failed before the fix and passed afterward. The unmodified fixture command then completed successfully on the native main-thread stack.

The implementation received author self-review. The approved plan restricts delegation without separate authorization; no independent reviewer was used.

## Browser and scale evidence

Chromium opened a fresh fixture after the stack fix with networking disabled. Internal next/back navigation, nested CSS, real fonts, and images worked. No scripts or external HTTP(S) requests ran. The fixture saved 7 resources using 9 HTTP requests and 177,049 decoded body bytes.

Synthetic discovery admitted at most 100,000 URLs from 100,002 links in 3,301 ms, with observed peak working set 39,563,264 bytes. A persisted 100,000-URL frontier checkpoint took 738 ms, produced 13,589,668 bytes, and observed peak working set 80,326,656 bytes. Overflow left the prior valid snapshot intact. These are backend measurements, not a 100,000-request network or desktop responsiveness benchmark.

## Implementation decisions and costs

- Existing dirty feature checkout and build cache were retained. Earlier README, installer, attribution, and upstream edits remain intact; mixed ownership prevented automatic feature commits.
- Capture requests run sequentially, below the global and per-origin ceilings. Throughput is lower than the planned concurrent ceiling permits.
- Each in-flight request reserves the remaining run budget durably. Uncertain crash recovery can exhaust that budget; explicit synchronization starts a fresh run.
- Proxy routing fails closed because proxy-side DNS cannot be verified. Proxy-dependent captures remain unavailable.
- A redirect to an origin without a checked robots policy is refused. Some otherwise permitted cross-origin redirects therefore fail.
- Raw bodies remain immutable; synchronization rebuilds offline output from raw data. Metadata is capped at 256 MiB and each rewritten document at 64 MiB.
- CSS input uses UTF-8. JavaScript rendering, form automation, browser-cookie extraction, and app-closed execution are outside this implementation.

## Acceptance still required

- Linux and macOS workspace execution and the six design acceptance scenarios on all supported platforms.
- Native credential-vault integration checks on each platform, including unavailable and locked vaults.
- Desktop and narrow-window keyboard, themes, cancellation, and restart smoke tests.
- Full 100,000-URL network/UI responsiveness acceptance and cross-volume destination checks.

Task 9 remains open until these checks have evidence. Existing Link Grabber and ordinary downloads remain separate workflows.

## Wizard UI update — 2026-11-25

The original scrolling form was replaced with a native, theme-driven four-step wizard inspired by the [IDM Grabber workflow](https://www.internetdownloadmanager.com/support/idm-grabber/grabber_wizard.html): project/template, save location, exploration, and file filters. Image/media/document presets populate actual extension filters. The folder picker runs off the UI thread. Existing project actions now live in a separate project workspace; optional credentials, schedules, and advanced rules remain available in expandable sections.

The app suite passed **337 tests, with 1 ignored**. This includes 10 Site Grabber tests, with new regressions for per-step validation, preserved Back navigation, image presets, and maximum resource size mapping. Formatting, translation extraction, and whitespace checks passed. The new native binary built successfully in an isolated cache after the shared build cache disappeared during Clippy. The retry of `cargo clippy -p sdl-app --all-targets --locked -- -D warnings` passed in that isolated cache.

Ten native screenshots were captured and inspected: the project workspace and all four wizard stages in both Fluent light and dark modes. Every screenshot contained the expected action control, and accessibility control bounds stayed within the window. Local artifacts are under `.superpowers/sdd/2026-11-25-site-grabber-ui/`.

Programmatic native button-click smoke testing was not executed: automatic approval review rejected the action as “blocked by policy” without a more specific reason. Back/Next behavior is covered by model tests; native visual inspection does not establish a full keyboard or interaction acceptance pass.

## Destination flow fix — 0.1.7

The reported flow selected `E:\` directly, saved successfully, then failed at Start because the destination was not empty. The wizard now treats that selection as a base folder and defaults to a safe project subfolder: project `skk` resolves to `E:\skk`. It shows the final path, checks it asynchronously before advancing from Step 2, and rechecks before saving metadata. Direct use of a selected folder remains available when that folder is empty.

Existing Draft, Paused, or Failed projects with no captured resources, generation, or charged/reserved bytes can use **Change destination**. This preserves capture settings and existing output files, then returns the project to Draft. Startup errors now show Failed; their pending schedule is suspended until manual retry. Genuine pause results remain Paused. Video guidance now explains the visible size limits without silently increasing them.

Verification on Windows, with one Cargo build job and locked dependencies:

- Full workspace: **915 passed, 22 ignored, 0 failed across 71 targets**.
- Focused Site Grabber app tests: **13 passed**. Recovery suite: **7 passed**.
- Workspace/all-target Clippy with `-D warnings`, formatting, translation extraction, and whitespace checks: passed.
- All **15 workspace packages** and their lockfile entries report **0.1.7**. The built executable's **ProductVersion and FileVersion both report 0.1.7**. Third-party dependency versions were retained.
- Four updated native previews (destination and filters in Fluent light/dark) were visually inspected; expected controls and window bounds passed checks.

Regressions cover the exact Windows drive-root path join, safe folder names including Windows device aliases, nonempty-parent preservation, file targets, read-only preflight, stale validation replies, failure status/schedule handling, and destination repair preserving settings while refusing partially charged projects. Tests used temporary directories; the supplied media server and the user's `E:\` contents were not modified or downloaded from.

Artifacts and logs: `.superpowers/sdd/2026-11-25-site-grabber-flow-fix/`. The verified executable is `.superpowers/sdd/2026-11-25-site-grabber-ui/build-cache/debug/swoopdl.exe`. This remains a local build; no installer was published, no tag was created, and earlier cross-platform/manual acceptance limits still apply.
