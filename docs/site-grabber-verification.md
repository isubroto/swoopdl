# Site Grabber verification

The implementation is present in the 0.1.7 source. Cross-platform acceptance is incomplete; this document does not certify a release.

Implemented scope includes saved projects, scoped static HTML/CSS crawling, robots policy, bounded decoded body accounting, durable pause/resume, offline generations, reports, conditional synchronization, calendar schedules, and optional memory-only or system-vault credentials. The native entry point is **Tasks → Site Grabber**.

## File workflow implementation — 2026-11-26

The current source adds a separate file catalog and checked normal-engine handoff. Verification artifacts for this run are under `.superpowers/sdd/2026-11-26-idm-site-grabber-parity/`. The six-task implementation plan is `docs/superpowers/plans/2026-11-26-idm-site-grabber-parity.md`.

Acceptance covers a local Flash-shaped directory graph, depth-one video discovery without full media GETs, false MIME/login pages, ignored Range and unsupported HEAD, selection/catalog recovery, safe destination names, idempotent dispatch after a lost acknowledgement, missing guards, and exact-byte pause/restart/resume through the normal engine. The live Flash check is separately ignored by default and must be invoked explicitly after local acceptance. It reads only directory pages and capped metadata prefixes, uses saved network settings, and has a 120-second cancellation watchdog.

The local workflow test scans three candidates, selects two, persists guarded jobs, pauses, reconstructs state after restart, and resumes both files to their exact 131,072-byte contents and original filenames. The unchecked file is absent. Additional regression cases cover immutable destination/policy records, lost enqueue acknowledgements, legacy-reader invisibility, oversized checkpoints, edited filters, scoped memory/vault references, checked signed-link refresh, DNS answers, redirects, and cross-origin header stripping.

Discovery-only acceptance against the supplied Flash URL used saved **System** routing without changing settings. The bounded run listed **47 video candidates, all 47 with known sizes**, consumed **816,013 scan bytes**, and downloaded **zero full episodes**. It stopped as **Limited** under the 120-second watchdog; this is useful candidate-discovery evidence, not complete enumeration. The run used depth 1, a 64-URL limit, a 2 MiB scan budget, and metadata prefixes capped at 16 KiB. Its destination was not created. Evidence is in `live-discovery/summary.json` and `live-check-output.log` beneath the artifact directory above.

Browser integration checks passed all 69 checks. Website validation passed 15 tests and built 85 pages offline, retaining the primary-language coverage requirement. Final Rust outcomes are recorded after completion below. Existing Windows routing evidence below describes the earlier offline implementation; eight HTML pages alone do not establish file-workflow parity. No release or publication is included in this work.

Native visual review used the current debug executable with isolated application data and offscreen windows. The results workspace and all four wizard pages were inspected at 1000×745. Controls and final save instructions fit without clipping; automatic downloading was unchecked. Five screenshots are retained in `native-review/`. These are synthetic preview states, not a manual sign-in, scheduling, or download-through-clicks acceptance run. The local workflow test supplies the scan/select/queue/restart/resume evidence. Linux/macOS acceptance and independent review remain unperformed; the plan requires author review without delegation.

## Windows automatic-discovery follow-up — 2026-11-26

The supplied URL, `http://203.76.96.50/ftp/TVseries/english/The%20Flash/`, was retested using saved System routing with browser inheritance enabled. Before the correction, it failed with zero resources because the transport categorically rejected Windows automatic discovery. A read-only WinHTTP probe returned error 12180: no discoverable WPAD configuration. The fix now resolves the route per URL, following the documented no-WPAD fallback while retaining separately configured static/environment proxy settings. Genuine proxies, explicit PAC scripts, unknown results, and other resolution errors remain refused. Automatic Windows credential logon is disabled during discovery.

The live System-mode capture now finishes **Completed**, saving the main listing and `s01/` through `s07/`: **8 HTML resources, 23,016 decoded body bytes, 0 pending URLs**. The isolated project allowed only in-scope directory listings and robots, with depth 1, a 128 KiB resource cap, a 2 MiB run cap, and a 120-second watchdog. No episodes were downloaded and saved application settings were not changed. Artifacts are under `.superpowers/live-system-fixed-2026-11-26/`, including the offline entry point, report, summary, and verification harness.

Focused regression checks pass for automatic DIRECT/no-WPAD versus proxy/failure outcomes, preserved static proxy fallback, redirect reevaluation, and route-resolution timeout/cancellation. The blocking-resolution regression was observed failing before the async-worker correction. `cargo test --workspace --locked -- --test-threads=1` passes with **934 passed, 0 failed, 22 existing manual tests ignored** across 72 suites. Website tests pass **15/15**; formatting and English-catalog checks pass. `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo build -p sdl-app --bin swoopdl --locked` pass. The rebuilt executable and symbols were placed in the original checkout's `target/debug`; both PE version fields are `0.1.7`, and the executable hash matches the build output. Builds use `CARGO_BUILD_JOBS=1`.

References: [WinHTTP AutoProxy functions](https://learn.microsoft.com/en-us/windows/win32/winhttp/winhttp-autoproxy-api) and [WinHttpGetProxyForUrl](https://learn.microsoft.com/en-us/windows/win32/api/winhttp/nf-winhttp-winhttpgetproxyforurl).

## Initial robots and routing repair — 2026-11-26

The repair is in `D:\Swoopdl\SwoopdlSource`, on the existing `main` checkout updated to the inspected 0.1.7 source. An isolated worktree protected the older checkout during development. The repair was then brought back at the user's request. Six pre-existing edited files were backed up and verified byte-for-byte after transfer. Existing website translation fixes were retained without lowering coverage requirements. No release was published or application network settings changed.

Windows validation uses Rust 1.99.1 and sequential builds with `CARGO_BUILD_JOBS=1`:

- `cargo test --workspace --locked -- --test-threads=1`: 932 passed, 0 failed, 22 existing manual tests ignored, across 72 test suites including documentation tests.
- `python -m unittest discover -s tools/website -p test_build.py -v`: 15 passed from the original checkout with its retained translation fixes.
- `cargo fmt --all --check` and `python tools/i18n/extract.py --check`: passed. No new translatable interface labels were needed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo build -p sdl-app --bin swoopdl --locked`: passed. The debug executable and symbols were placed in the original checkout's `target/debug`. `swoopdl.exe` has FileVersion and ProductVersion `0.1.7`; its SHA-256 matches the build output.

The local encoded-space directory fixture captures three resources, including a tiny video payload, and charges exactly 154 decoded body bytes. Parent-directory navigation is refused. The real app's default proxy selection is exercised with a deterministic verified-DIRECT resolver; tests do not alter environment variables or Windows proxy settings.

Regression coverage includes per-hop system-route decisions, proxy bypass rules, refused proxy redirects, permanent configuration failures without repeated attempts, empty and real robots exclusions, HTTP 401/403/404/503, Retry-After, connection failures, header and body timeouts, decoding and file errors, byte limits, pause, retained seeds, retry after configuration correction, capped reports, credential/query redaction, and failed-schedule suspension. Native project state retains the useful cause after completion and reopening. Real robots exclusions still prevent the destination request.

A read-only probe found saved System inheritance and Windows automatic discovery enabled, without a static proxy or PAC URL. This initial repair still refused automatic discovery and required an explicit-direct workaround. The follow-up above removes that restriction for Windows DIRECT/no-WPAD outcomes; changing saved routing settings is no longer needed for the tested URL.

System DIRECT routing is supported only when configuration can be verified. Actual proxy routes, unknown configurations, and macOS system routing remain conservatively refused; explicit direct routing remains available. Linux/macOS acceptance and native click automation were not performed for this repair. No episodes were downloaded during verification. Review was performed by the author, following the approved instruction not to delegate.

## Earlier feature baseline

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
