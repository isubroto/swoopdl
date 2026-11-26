# IDM-style Site Grabber Implementation Plan

> **For agentic workers:** Use `executing-plans` inline after design approval. Track each task with checkboxes. Work in `D:/Swoopdl/SwoopdlSource`; do not create another worktree or delegate. This document is a proposed plan, not permission to implement, commit, publish, change global settings, or download episodes.

**Goal:** Provide explore, review, and selected-file downloading through the normal Swoopdl engine, while retaining offline mirroring.

**Architecture:** Separate bounded discovery from a durable candidate catalog and guarded queue handoff. Reuse existing download/queue machinery; do not build a second segmented downloader. Keep offline publication behind its explicit mode.

**Tech Stack:** Existing Rust workspace, Tokio, checked HTTP transport, HTML/CSS discovery, egui/native theme, main download engine, and queue.

**Spec:** `docs/superpowers/specs/2026-11-26-idm-site-grabber-workflow.md` — proposed, awaiting review.

## Consolidation status (2026-11-26)

`D:/Swoopdl/SwoopdlSource` is authoritative. All 13 outside changes were already identical or superseded here. Original source bytes were preserved. External edits, staged/unstaged patches, and ignored artifacts are archived under `.superpowers/consolidation-2026-11-26/`; the external build cache is now `target/grabber-fix-cache`. The outside worktree was unregistered; Windows retained only its empty directory. This is consolidation and planning, not implementation of these tasks.

## Why the current implementation is insufficient

| Gap | Evidence | Effect |
| --- | --- | --- |
| No explore/review/selected-download phase | `site_grabber.rs::SiteAction` has Start/Pause/Sync, but no candidate selection or queue handoff. | Start captures immediately instead of letting users choose files. |
| Anchors always become pages | `discover.rs::attributes`: `a`/`area` href uses `ResourceKind::Page`. | Episode links can be rejected by page depth or treated as pages. |
| Page-role links bypass file selection | `policy.rs::save_allowed` returns true for every Page. | Video/download anchors can evade extension/type rules. |
| Filtering does not prevent full transfer | `crawl.rs::run_inner` requests bodies before final MIME classification; publication applies save rules later. | Unwanted or oversized assets may be fetched before selection. |
| Mirror transfer behavior | `crawl_fetch.rs` uses bounded single GET; resume restarts a body. | No normal segmented/range-resume pipeline for selected files. |
| Mirror output names | `paths.rs::local_path` uses hash names and generic `.bin` fallback. | Not the expected named episodes in original/flat/category folders. |
| Incomplete filters and result model | `ProjectConfig` has prefixes/extensions/MIME, and `ProjectSnapshot` has resources, but no checked candidate catalog or min/max file filter. | No IDM-style result tree, selected byte totals, or selection persistence. |

The successful eight-page live test is routing evidence only. It is not proof that video selection/download works.

## Global constraints

- Original folder only. Preserve all current dirty work, version 0.1.7, and the Windows System-routing fix. Version/release decisions require separate user direction.
- `CARGO_BUILD_JOBS=1`; Cargo builds run sequentially. Start with focused checks, then broaden once the workflow passes.
- No full episode downloads during automated/live discovery verification. Use local tiny files and explicit probe/scan limits.
- Main download jobs must retain capture policy at connection time, including retries, redirects, DNS/address checks, session boundaries, and current proxy limitations.
- Existing snapshots open as offline projects; preserve their output and recoverable backup. No silent migration into automatic file downloads.
- Keep the native theme, accessibility, translations, and website coverage threshold. No unrelated UI redesign or dependency upgrades.

## Review focus

1. A linked `.mkv` on the last allowed HTML level must be a file candidate, not a forbidden deeper page. Task 1 pins this.
2. A server ignoring HEAD/Range, lying about MIME/length, or returning a login page must not cause an unselected full transfer. Task 1 pins this.
3. A queued URL changing destination, credentials, or DNS after discovery must not escape capture constraints. Task 4 pins this.
4. Double-click, crash between enqueue and acknowledgement, or rescan must not create duplicate jobs or lose selection. Tasks 2/4 pin this.
5. Duplicate names, encoded separators, Windows reserved names, and existing files must not overwrite unrelated data. Task 5 pins this.

## Task 1 — Correct discovery, classification, and metadata-only scanning (P0)

**Files:** modify `crates/grabber/src/discover.rs`, `policy.rs`, `model.rs`; create `crates/grabber/src/explore.rs`, `classify.rs`; extend `crates/engine/src/crawl_fetch.rs`; tests in `crates/grabber/tests/exploration.rs` and `crates/labserver/src/crawl.rs`.

**Interfaces:** introduce `LinkRole::{Navigation, File, Unresolved}`; `CandidateMetadata` with final URL, name, optional MIME/size, and source/referrer; `ExploreEvent::{PageVisited, CandidateFound, Skipped, Stopped}`. Add a checked `probe_crawl` metadata operation beside `fetch_crawl`, using the same URL/address/routing/credential controls.

- [ ] Write `depth_one_directory_scan_lists_videos_without_downloading_them`, `file_links_do_not_consume_page_depth`, `file_patterns_apply_to_anchor_downloads`, and `html_disguised_as_video_is_not_a_video` against a local Flash-shaped fixture.
- [ ] Assert root/season HTML is read, parent navigation is refused, MKV/MP4 candidates exist, unrelated files are excluded, and **zero full media GETs** occur before download selection.
- [ ] Run `cargo test -p sdl-grabber --test exploration --locked`. Expected: new assertions fail under the current immediate-mirror behavior.
- [ ] Implement classification before page-recursion/file-filter decisions. Preserve unknown cases until metadata is available. Apply depth only to HTML navigation.
- [ ] Implement HEAD plus capped metadata fallback, maximum 16 KiB per fallback body. Do not reuse `probe_kept` blindly: its first plain GET/retained response is designed for downloading, not a scan-only guarantee. Cover ignored Range, absent length, redirects, cancellation, and false MIME.
- [ ] Rerun the focused suite. Expected: eligible candidates appear with no unselected media transfer; checked transport restrictions remain intact.

## Task 2 — Durable candidates and independent exploration/transfer state

**Files:** modify `crates/grabber/src/model.rs`, `store.rs`, `lib.rs`, `sync.rs`, `report.rs`; create `crates/grabber/src/catalog.rs`; test `crates/grabber/tests/catalog.rs`, `recovery.rs`.

**Interfaces:** `ProjectMode::{Files, OfflineSite}`, separate `ExploreStatus`/`TransferStatus`, and a catalog keyed by existing `UrlKey`. `CandidateRecord` includes metadata, checked state, status/reason, and optional main-item association. Task 1 events update this catalog; UI/queue tasks consume it.

- [ ] Add failing tests for candidate merge, saved selection, stop/resume, exact URL deduplication, rescan, and opening version-1 offline snapshots.
- [ ] Keep existing offline `ProjectSnapshot` and version-1 store unchanged. Add a separate versioned file-project catalog namespace, atomic writes, and recoverable backups. A mode/envelope selects the store; existing projects remain OfflineSite. Reject unknown newer formats explicitly without reverting to stale state.
- [ ] Preserve signed URL identity while redacting query values/userinfo from logs and reports. Keep session secrets in current memory/vault facilities.
- [ ] Run `cargo test -p sdl-grabber --test catalog --test recovery --locked`. Expected: selection and recovery survive restart; no download starts from loading a project.

## Task 3 — Native results workspace and complete wizard controls

**Files:** modify `crates/app/src/site_grabber.rs`, `site_grabber/view.rs`, `site_grabber/wizard.rs`; create `site_grabber/results.rs`; regenerate `i18n/en.tsv` and necessary primary website catalog entries.

**Interfaces:** add `SiteAction::{StartExploring, StopExploring, CheckCandidates, DownloadSelected, AddSelectedToQueue}` using stable project/candidate IDs from Task 2. Preserve offline actions and explicit mode choice. Task 3 emits IDs only; Task 4 resolves guarded transfer requests.

- [ ] Add state tests proving Explore does not download, check/uncheck changes selection only, selected totals handle unknown sizes, and stop exploration does not cancel already queued downloads.
- [ ] Keep four wizard steps: project/template/session; save layout; page-exploration rules; file-download filters. Add separate scopes, wildcard patterns, min/max size, and an explicit auto-download toggle defaulting off.
- [ ] Add a theme-matched tree/list with checkboxes, name, type, size/unknown, source, URL, and independent statuses. Provide Start/Stop Exploring, Check/Uncheck All, Download checked, Add to queue, and Edit project.
- [ ] Virtualize large result lists; preserve checked state across sort/filter. Rule edits create a new scan revision and do not silently retarget queued jobs.
- [ ] Run `cargo test -p sdl-app --bin swoopdl site_grabber --locked -- --test-threads=1` and `python tools/i18n/extract.py --check`. Expected: mode/selection actions pass and catalog is current.

## Task 4 — Guarded normal-engine downloads and queue handoff (P0)

**Files:** create `crates/app/src/site_grabber_jobs.rs`; modify `crates/app/src/app.rs`, `model.rs`, `queue.rs`; extend `crates/engine/src/download.rs` and checked resolver/connector paths as needed; test `crates/grabber/tests/queue_handoff.rs` plus app/engine integration tests.

**Interfaces:** `SiteDownloadRequest` carries `project_id: ProjectId`, `candidate_id: UrlKey`, `scan_revision: u64`, `url: Url`, `name: String`, `destination: PathBuf`, `expected_bytes: Option<u64>`, `source_page: Url`, an immutable capture-policy snapshot, and a scoped credential reference. Persist a dispatch identity and resulting `item_id: u64`. Reuse `App::add_grabbed`, `App::push_item`, `App::request_parts`, `start_queue`, and normal queue behavior through a small shared enqueue adapter. Do not reopen Link Grabber for another selection dialog.

- [ ] Write failing tests for exactly two checked candidates producing exactly two jobs; unchecked entries produce none; restart/double-click does not duplicate jobs.
- [ ] An optional runtime guard distinguishes ordinary jobs, but is mandatory for every Site Grabber job. Persist guarded jobs in a separate versioned store, never as unrestricted legacy records. Older binaries must not discover or resume them through the ordinary queue store. Rebuild runtime items only after validating the immutable policy and scoped credential reference; missing guards fail closed. Recheck every connection, segment, retry, redirect, resolved address, route, robots policy, and credential boundary. Ordinary jobs retain current behavior.
- [ ] Never copy a scan-time approval into an unrestricted normal job. Preserve selected proxy routing rather than silently changing it; unsupported constrained routes report a useful failure.
- [ ] Reuse segmentation, overlap/range resume, disk checks, configured connection counts, bandwidth limits, and existing queues. Separate scan budgets from optional transfer size limits; no hidden 256 MiB/1 GiB mirror ceiling on episode jobs.
- [ ] Persist dispatch identity, destination reservation, immutable guard, and job state before exposing a runtime item. Reconcile by dispatch identity after restart. Deleting project metadata must retain guards on queued/running jobs until cancellation or completion. Test downgrade visibility, missing guards, and deleted project metadata.
- [ ] Cover crash between enqueue/acknowledgement, expired session/URL, Range unsupported, changed ETag, redirected private address, and cross-origin header stripping.
- [ ] Run targeted app Site Grabber and engine/grabber integration suites. Expected: selected jobs use the existing engine, resume normally, and cannot escape capture policy.

## Task 5 — Save layouts, project editing, and schedule actions

**Files:** create `crates/grabber/src/destinations.rs`; modify `paths.rs`, `sync.rs`, `model.rs`, app `site_grabber.rs`/`site_grabber_jobs.rs`; tests in `crates/grabber/tests/destinations.rs`, `sync.rs`, `offline.rs`.

**Interfaces:** `SaveLayout::{Flat, OriginalSubfolders, Category}`, explicit collision policy, and `ScheduledAction::{ExploreOnly, ExploreAndDownload, DownloadChecked, Synchronize}`. Keep current offline hashed paths and atomic rewriting for OfflineSite mode.

- [ ] Add failing tests for original episode names/season folders, same-name collisions, encoded traversal, Windows device names, existing files, and safe overwrite choices.
- [ ] Accept an existing nonempty Downloads folder in Files mode. Validate each reserved path and apply explicit per-file collision handling. Keep empty-directory/publication-root rules scoped to OfflineSite. Test that unrelated existing files remain unchanged.
- [ ] Use existing naming/destination checks for file exports; preserve source-relative directories beneath the chosen root. File mode must not require publishing an offline index or exporting every navigation page.
- [ ] Permit editing a stopped project's filters/save preferences for future exploration/unqueued candidates. Never move completed files or rewrite active job paths implicitly.
- [ ] Persist selected candidates and scheduled action. Keep manually paused projects paused, suspend failed scheduled runs, and avoid replaying every missed schedule. Make app-open execution requirements explicit.
- [ ] Run focused destination/sync tests plus existing offline suites. Expected: selected file layout is usable and old offline projects still publish/recover correctly.

## Task 6 — Capability boundaries and end-to-end acceptance

**Files:** existing browser capture/import bridge, `crates/app/src/grabber.rs`, `transfer.rs`, media handoff UI; `docs/site-grabber.md`, `docs/site-grabber-verification.md`, `CHANGELOG.md`; local fixtures and workflow tests.

- [ ] First prove the complete local workflow: scan without full media transfer, review, select two tiny files, queue, pause, restart, resume, and verify exact output bytes and filenames.
- [ ] Repeat the supplied Flash URL as discovery-only under saved System settings. Acceptance is **listed matching video candidates with useful metadata**, not merely eight HTML pages. Do not download full episodes automatically.
- [ ] Add explicit browser-rendered-link import using existing authorized browser context, keeping source/referrer association. Treat sandboxed browser rendering/JavaScript execution as a separate follow-up, not a hidden default dependency.
- [ ] Label unsupported/different features accurately: dynamic pages, authenticated sessions without a usable context, HLS/DASH handoff, DRM, proxy-dependent capture, FTP traversal, subdomain expansion, and app-closed scheduling. Offer existing media/import tools where appropriate; do not execute commands from the supplied reference.
- [ ] Record saved custom templates, independent concurrency controls, richer statistics/file properties, legacy cache behavior, and optional completion actions as later parity items. Do not imply these exist because the first explore/select/download milestone works.
- [ ] Run fmt, English extraction, website tests, affected suites, workspace Clippy/all-targets, and the Windows workspace suite sequentially. Update translations without relaxing coverage.
- [ ] Update docs/changelog around the final workflow and retain a local reviewable build. Record automated versus native/manual verification separately. No release/publication or post-completion programs/power actions are part of this plan.

## Delivery order and stop conditions

1. Tasks 1–2: trustworthy candidate discovery and durable selection data.
2. Tasks 3–4: first usable IDM-style explore/review/download slice. This is the first parity milestone.
3. Task 5: complete file-save/edit/schedule behavior.
4. Task 6: acceptance, clear limitations, and explicit import/media integration. Browser rendering and other deferred subsystems require separate plans.

Do not claim IDM-style completion until Tasks 1–5 and the local end-to-end acceptance pass. Do not claim full IDM feature parity from documentation alone. No implementation or build was performed while preparing this plan.
