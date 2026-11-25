# Full Site Grabber Implementation Plan

Implementation status: the feature code is present in the existing feature checkout. See [verification evidence](../../site-grabber-verification.md) for current tests, implementation decisions, and remaining acceptance. Cross-platform and manual desktop acceptance remain open; unchecked historical steps must not be read as a release certification.

> **For agentic workers:** Use the executing-plans skill to implement this plan task by task. Steps use checkbox syntax for tracking. Delegate only when separately authorized by the user or applicable repository instructions.

**Goal:** Add controlled, saved website capture projects with offline pages, pause/resume, reports, duplicate detection, and periodic synchronization.

**Architecture:** Add a UI-independent `sdl-grabber` crate. Add a bounded engine transport adapter that shares SDL network settings and limits, and integrate project management through a separate egui dialog. Preserve the existing Link Grabber and ordinary download behavior.

**Tech Stack:** Existing Rust/Tokio/reqwest/serde/craft-atomic/egui stack; proposed LOL HTML and a maintained CSS tokenizer, subject to the dependency gate in Task 1.

**Spec:** [Full Site Grabber design](../specs/2026-11-24-full-site-grabber-design.md). Read it before executing any task. This is a proposed implementation plan, not an implemented feature or a user-approved dependency list.

## Global constraints

- Preserve the pinned Rust toolchain `1.99.1`, workspace edition `2024`, and declared Rust version floor `1.95`. Verify new dependencies support the declared floor, or resolve that conflict before adding them.
- Retain GPL-3.0-or-later licensing and required third-party notices. Use Swoopdl/SDL names and existing Swoopdl assets.
- Support Windows, Linux, and macOS. Run heavy local builds sequentially; start with `CARGO_BUILD_JOBS=1` on the user's 16 GB machine.
- Defaults: depth 3; 1,000 unique URLs, configurable up to 100,000; 1 GiB run body budget; 256 MiB resource cap; 8 MiB parse cap; 1 MiB robots cap.
- Defaults: 4 active requests total, 2 per origin; 1 request/second per origin; 3 attempts with 1/2-second backoff; 10 redirects; 30-second connect/read timeouts. Honor longer Retry-After.
- Count decoded body bytes across all workers, retries, robots requests and restarts. Use one stream per resource and the existing shared bandwidth/host limits.
- Default to exact-origin/seed-directory page scope, same-origin assets, public unauthenticated HTTP(S), and one active project.
- No JavaScript rendering, form submission, credential extraction, plaintext credential fallback, or access-control bypass. No SQLite migration or replacement of Link Grabber.
- Reaching a budget produces `Limited`, never an unqualified complete result. Do not mark checklist items complete without their verification evidence.

## Review focus

1. Windows case folding, reserved names and reparse points must not merge unrelated URLs or escape the output directory. Pin in Task 5.
2. A host resolving to a different address after policy inspection must not reach a forbidden private destination. Pin in Task 2.
3. Gzip/chunked bodies, unknown lengths and concurrent retries must not exceed the decoded byte budget. Pin in Task 2.
4. Corrupt metadata, a disconnected destination and power loss between body and manifest writes must retain a recoverable project. Pin in Task 4.
5. A 304 with a missing cached body, or identical HTML served from different base URLs, must not create missing or wrongly rewritten offline pages. Pin in Tasks 5 and 7.

## File and responsibility map

All new names below are proposed interfaces, not existing APIs. Keep modules private unless listed as a crate boundary. Inspect existing module conventions before editing; do not refactor unrelated code.

| Files | Responsibility |
| --- | --- |
| `Cargo.toml`, `Cargo.lock`, `crates/grabber/Cargo.toml` | Workspace dependency wiring, approved parser versions and lockfile. |
| `crates/grabber/src/{lib,model,policy}.rs` | Public project model, limits, URL identity, scope and filter decisions. |
| `crates/engine/src/{lib,download,net}.rs`, new `crates/engine/src/crawl_fetch.rs` | Bounded transport using existing connector, redirect/header and limiter behavior. |
| `crates/grabber/src/{robots,discover,crawl}.rs` | Robots policy, structural discovery, bounded frontier and crawl coordination. |
| `crates/grabber/src/{store,paths,rewrite,report,sync}.rs` | Recovery, output mapping, offline generations, reports and conditional sync. |
| `crates/grabber/tests/{policy,transport,crawl,recovery,offline,sync}.rs` | Deterministic acceptance fixtures and adversarial cases. |
| `crates/labserver/src/{lib,server}.rs`, new `crates/labserver/src/crawl.rs` | Small crawl graph/fault fixtures and request logs; extend the existing server rather than starting a second test framework. |
| `crates/app/Cargo.toml`, `crates/app/src/{main,app,site_grabber}.rs` | Project UI, runtime bridge, menu commands and scheduling integration. |
| `crates/app/src/{queue,secrets}.rs` | Reuse schedule calculations and protected credential resolution only where required. |
| `i18n/en.tsv`, `docs/site-grabber.md` | Generated English catalog entries and user-facing usage/limitations. Confirm the catalog path in the extractor before editing. |

## Task 1: Project model and URL policy

**Files:** Create the grabber crate, `model.rs`, `policy.rs`, `lib.rs`, and `tests/policy.rs`; modify workspace manifests/lockfile. Add any required third-party notices to `ATTRIBUTION.md` after inspecting the selected versions' licenses.

**Interfaces:**

- Define serializable `ProjectConfig` with name, seed `Url`, destination `PathBuf`, page/resource origin sets, path prefix, include/exclude prefixes, MIME/extensions, and `Limits` with the spec's defaults.
- Define `ProjectId` as a validated opaque string, `UrlKey` as a SHA-256 digest of the normalized fetch URL, `ResourceKind::{Page, Asset}`, `Decision::{Allow, Skip(SkipReason)}`, and `ProjectStatus::{Draft, Running, Pausing, Paused, Completed, Limited, Failed}`.
- Expose `normalize_url(base: &Url, reference: &str) -> Result<Url, PolicyError>` and `evaluate(config: &ProjectConfig, url: &Url, kind: ResourceKind, depth: u16) -> Decision`.
- Define `ProjectSnapshot` containing config, status, bounded resource records/frontier, body-byte counters, report records and optional next-run time. Give the persisted format an explicit version.

- [ ] Write failing `normalization_preserves_query_identity`, `scope_separates_pages_and_assets`, `exclude_wins`, and `seed_depth_is_zero` tests. Assert fragments collapse fetch identity, query order remains significant, ports/origins are distinct, asset origins do not grant navigation, and depth 4 is rejected with default depth 3.
- [ ] Run `cargo test -p sdl-grabber --test policy`; confirm the expected missing-crate/API failure before implementation.
- [ ] Resolve parser dependencies using current official documentation. Verify Rust compatibility and licenses, select exact versions, configure parser/encoding memory caps, and add the crate/model/policy. Validate URL userinfo, unsupported schemes, zero/overflow limits and the 100,000 URL ceiling.
- [ ] Rerun the policy tests and `cargo check -p sdl-grabber --locked`; require passing output.
- [ ] Review and commit only this task's files with intent `feat: define site capture projects and URL policy`.

## Task 2: Engine transport with enforceable budgets

**Files:** Add `crates/engine/src/crawl_fetch.rs`; modify engine exports and only the necessary connector/redirect internals. Add `crates/grabber/tests/transport.rs` and crawl HTTP fixtures in labserver.

**Interfaces:**

- Add engine `CrawlRequest` containing URL, origin-scoped headers and a temporary destination; `CrawlLimits` for body cap/timeouts/redirect cap; and shared `BodyBudget` with atomic checked reservations and a durable consumed counter supplied by the caller.
- Add a `CrawlAccess` policy interface that checks each URL and each resolved peer before connection. Production blocks unauthorized private addresses; tests inject explicit loopback permission.
- Expose `fetch_crawl(request: CrawlRequest, options: Options, limits: CrawlLimits, budget: Arc<BodyBudget>, access: Arc<dyn CrawlAccess>, cancel: CancellationToken) -> Result<CrawlResponse, CrawlFetchError>` as an async function. `CrawlResponse` carries final URL, redirect aliases, status, MIME, ETag/Last-Modified, bytes consumed and optional temporary body path. Return ordinary HTTP statuses to the coordinator for policy/retry decisions.
- Preserve existing `fetch`, `Download`, `http_client`, and normal redirect behavior. Crawler requests have controlled manual redirects and one stream per body.

- [ ] Write failing tests `redirect_checked_before_request`, `credentials_do_not_cross_origin`, `dns_rebinding_peer_is_rejected`, `decoded_chunked_body_obeys_budget`, and `parallel_retries_share_budget`. Assert forbidden endpoints receive zero requests, decompressed bytes count, and no reservation exceeds the configured budget.
- [ ] Run `cargo test -p sdl-grabber --test transport`; verify the intended failure.
- [ ] Implement the adapter using existing engine proxy/DNS/adapter/TLS and shared limiter plumbing. Add a connector enforcement hook where needed; a URL precheck alone is insufficient. Prevent automatic uncontrolled redirects or hidden retries. Stream bodies to temporary files, enforce both per-resource and aggregate caps, and surface progress for durable accounting.
- [ ] Rerun transport tests and engine tests: `cargo test -p sdl-engine --locked`. Add cancellation-during-read/backoff and HTTPS-downgrade credential tests; require all applicable tests pass.
- [ ] Review the isolated transport diff and commit with intent `feat: add bounded engine transport for site capture`.

## Task 3: Controlled crawl and structural discovery

**Files:** Add `robots.rs`, `discover.rs`, `crawl.rs`, and `tests/crawl.rs`; extend labserver crawl fixtures.

**Interfaces:**

- Define `DiscoveredLink { url: Url, kind: ResourceKind, fragment: Option<String> }` and `discover(body: &[u8], mime: &str, final_url: &Url) -> Result<Vec<DiscoveredLink>, ParseError>`, with the spec's parse cap and supported attributes/CSS constructs.
- Define `CrawlCommand::{Pause, Cancel}` and bounded `CrawlEvent` messages carrying counters/status/errors, not full page bodies.
- Expose async `run(config: ProjectConfig, state: ProjectSnapshot, transport: CrawlTransport, commands: mpsc::Receiver<CrawlCommand>, events: mpsc::Sender<CrawlEvent>) -> Result<ProjectSnapshot, CrawlError>`. `CrawlTransport` wraps Task 2 engine options, shared budget and access policy. Durable checkpointing is added in Task 4.
- `robots.rs` supplies per-origin cached policy for token `SwoopdlGrabber`, with RFC matching/error handling and the specified conservative 401/403 restriction.

- [ ] Write failing `crawl_loop_is_bounded`, `robots_disallow_never_requested`, `robots_redirect_does_not_expand_scope`, `nested_css_and_srcset_are_discovered`, and `filters_do_not_accidentally_block_traversal` tests.
- [ ] Run `cargo test -p sdl-grabber --test crawl`; confirm the intended failures.
- [ ] Implement a breadth-first bounded frontier, URL/alias deduplication, page-depth tracking, per-origin pacing, global/per-origin permits and status-specific retries. Count discoveries before admitting them, including skipped URLs. Apply policy before request and after every redirect. Parse only supported capped HTML/CSS bodies; do not execute scripts.
- [ ] Add deterministic-clock tests for Retry-After, request spacing, 404/no retry, 503/three attempts, robots longest-match/Allow ties, 4xx/5xx handling, caching and cancellation. Rerun crawl/transport suites; require the request log to prove every bound.
- [ ] Commit with intent `feat: crawl scoped site links with robots and limits`.

## Task 4: Durable projects and pause/resume

**Files:** Add `store.rs` and `tests/recovery.rs`; extend `model.rs` and `crawl.rs`.

**Interfaces:**

- Add `ProjectStore::open(root: &Path, id: &ProjectId) -> Result<ProjectStore, StoreError>`, `load(&self) -> Result<ProjectSnapshot, StoreError>`, and `checkpoint(&self, state: &ProjectSnapshot) -> Result<(), StoreError>`.
- Pass an owned `ProjectStore` into `run`; checkpoint transitions and frontier/body-budget updates. Use existing craft-atomic patterns and backup recovery, separate from the application's normal list store.
- Define `reconcile(state: ProjectSnapshot, output_root: &Path) -> Result<ProjectSnapshot, StoreError>` to recover interrupted requests, unpublished bodies and missing output files without resetting consumed budgets.

- [ ] Write failing `pause_ack_follows_checkpoint`, `restart_requeues_inflight_once`, `corrupt_latest_uses_backup`, `missing_destination_is_actionable`, and `crash_between_body_and_manifest_is_recoverable` tests using temporary directories and injected write failures.
- [ ] Run `cargo test -p sdl-grabber --test recovery`; verify failure before implementation.
- [ ] Implement versioned atomic state/backup writes and durable byte accounting. Pause stops admission, cancels active work and persists state before acknowledging. Treat partial bodies as restartable full fetches initially. Bound snapshots/report records by the admitted URL limit.
- [ ] Run recovery and crawl suites, including a budget test interrupted immediately before checkpoint. Prove recovery charges any uncertain in-flight reservation conservatively instead of replenishing the budget.
- [ ] Commit with intent `feat: persist and resume site capture projects`.

## Task 5: Safe paths, offline rewriting and reports

**Files:** Add `paths.rs`, `rewrite.rs`, `report.rs`, and `tests/offline.rs`; extend project records with raw hashes and published mappings.

**Interfaces:**

- Add `local_path(key: &UrlKey, mime: &str) -> Result<PathBuf, PathError>` for bounded hash-based relative paths and a checked writer confined to the project root.
- Define `ResourceMap` mapping successful URL keys/redirect aliases to local paths, and `rewrite_document(body: &[u8], mime: &str, base: &Url, map: &ResourceMap) -> Result<Vec<u8>, RewriteError>`.
- Add `publish_generation(store: &ProjectStore, resources: &ResourceMap) -> Result<PathBuf, PublishError>` and streaming `write_report(state: &ProjectSnapshot, writer: &mut dyn Write) -> Result<(), ReportError>`. Reports contain sanitized URL/source/status/reason records, never secret headers or sensitive query values.

- [ ] Write failing `offline_fixture_has_no_required_network_fetches`, `windows_case_and_reserved_names_do_not_collide`, `symlink_escape_is_rejected`, `same_raw_html_different_base_rewrites_separately`, and `uncaptured_link_keeps_absolute_target` tests.
- [ ] Run `cargo test -p sdl-grabber --test offline`; confirm intended failures.
- [ ] Implement safe file creation, immutable raw bodies, raw-content SHA-256 duplicate detection, HTML/CSS/srcset rewriting, base removal, fragment retention, sanitized broken-link reports and offline script/form restrictions. Keep per-URL rewritten pages separate even when raw bytes match. Publish only complete generation mappings atomically.
- [ ] Run offline/recovery suites. Serve or open the published fixture in a test browser with network disabled; require working internal navigation, CSS, fonts and images, and no automatic external requests. Test cross-volume destination behavior and the Windows reparse-point path on Windows.
- [ ] Commit with intent `feat: publish offline site copies and crawl reports`.

## Task 6: Native project wizard and progress controls

**Files:** Add `crates/app/src/site_grabber.rs`; modify app manifest, `main.rs` and `app.rs`; add localized strings through the existing extractor. Add focused unit tests alongside `site_grabber.rs` for its command/state conversion.

**Interfaces:**

- Define UI `SiteGrabber` with project list, wizard draft and bounded progress state; `show(&mut self, ctx: &egui::Context) -> Outcome<SiteAction>` using the app's established dialog conventions.
- Define `SiteAction::{Create(ProjectConfig), Start(ProjectId), Pause(ProjectId), Synchronize(ProjectId), OpenOutput(ProjectId), ExportReport(ProjectId), DeleteMetadata(ProjectId)}`. The app loop resolves paths/network settings and dispatches work off the UI thread.
- Add a distinct `Cmd::SiteGrabber` and Tasks menu entry. Retain `Cmd::LinkGrabber` and its existing behavior.

- [ ] Write failing model tests proving invalid seed/destination/limits cannot emit Create, Start cannot run two projects, Pause is pending until its durable acknowledgment, and metadata deletion does not delete captured files.
- [ ] Run `cargo test -p sdl-app site_grabber -- --test-threads=1`; verify the expected initial failure.
- [ ] Implement the wizard and project list with scope/budget review, accessible labels, theme support and explicit Limited/Failed reasons. Bind Start/Pause/Resume/Open/Report to Tasks 3–5. Never block egui on network, hashing or disk writes.
- [ ] Run focused UI model tests and `python tools/i18n/extract.py`, then `python tools/i18n/extract.py --check`. Smoke-test keyboard navigation, cancel, app restart and multiple themes at desktop and narrow window sizes. Confirm generated catalog paths before staging.
- [ ] Commit with intent `feat: add site grabber project management UI`.

**First usable milestone:** After Task 6, a user can configure and save a bounded public static-site capture, pause/restart it, browse its offline copy and inspect its report. Do not claim the periodic synchronization requirement is finished yet.

## Task 7: Conditional and periodic synchronization

**Files:** Add `sync.rs` and `tests/sync.rs`; extend store/model/UI/app integration. Reuse existing queue schedule calculations; change `queue.rs` only if a small shared time helper must be exposed.

**Interfaces:**

- Add async `synchronize(store: ProjectStore, transport: CrawlTransport, commands: mpsc::Receiver<CrawlCommand>, events: mpsc::Sender<CrawlEvent>) -> Result<ProjectSnapshot, CrawlError>`.
- Add `due_run(state: &ProjectSnapshot, now: SystemTime) -> Option<ProjectId>` and a persisted schedule configuration using existing daily/weekly/one-time time semantics. Only one run may be dispatched for a due project.

- [ ] Write failing `sync_304_uses_verified_body`, `sync_304_missing_body_refetches`, `changed_css_rebuilds_dependents`, `interrupted_sync_keeps_previous_generation`, `overdue_schedule_runs_once`, and `manual_pause_suppresses_schedule` tests.
- [ ] Run `cargo test -p sdl-grabber --test sync`; confirm the intended failures.
- [ ] Implement ETag/Last-Modified requests, changed-page rediscovery, hash-based unchanged detection, atomic generation publication, non-destructive missing-resource reporting and persisted schedule markers. Apply fresh per-run limits while retaining interrupted-run accounting. Integrate with the existing app tick/runtime; app-closed sync waits until reopening.
- [ ] Rerun sync/offline/recovery suites and test local clock changes, daylight-saving transitions and repeated due ticks. Require one active run and a usable previous capture after injected failure.
- [ ] Commit with intent `feat: synchronize saved site capture projects`.

## Task 8: Protected optional authentication

**Files:** Extend `crates/app/src/site_grabber.rs`, app secret integration, grabber project credential references and engine policy tests. Keep raw credentials out of the grabber's serialized model.

**Interfaces:**

- Add `CredentialRef` containing only a protected-store identifier and exact allowed origin. The app resolves it into transient Task 2 request headers immediately before running.
- Provide memory-only credentials when the user chooses them. Refuse persistent storage if protection cannot be verified; do not call the existing plaintext fallback as a substitute.

- [ ] Write failing tests proving serialized projects/reports contain no secret values, protected-store failure cannot persist a password, restart with a locked vault pauses for user action, and redirects never expand credential scope.
- [ ] Run focused app credential tests and grabber transport tests; confirm intended failures.
- [ ] Implement explicit public/authenticated project selection and protected references, respecting existing website/session permissions. Keep the authentication UI optional; no browser-cookie scraping or login-form automation.
- [ ] Rerun tests with a fake unavailable vault plus applicable native vault tests on each platform. Inspect generated files/logs for the fixture secrets; require no matches.
- [ ] Commit with intent `feat: support protected site capture credentials`.

## Task 9: Cross-platform acceptance and user guidance

**Files:** Add `docs/site-grabber.md`; update `docs/FEATURE_GAPS.md` only after acceptance. Add source/guide navigation entries where the existing documentation indexes require them. Modify workflow files only if a required new test is not already covered by workspace checks.

- [ ] Document project creation, scope/asset-origin choices, depth and byte limits, offline browsing, pause/restart, reports, public/protected sessions and periodic sync. State JavaScript and app-closed scheduling limitations clearly.
- [ ] Run `cargo fmt --all --check`, `python tools/i18n/extract.py --check`, and `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- [ ] Run `cargo test --workspace --locked -- --test-threads=1` on Windows and `cargo test --workspace --locked` on Linux/macOS, sequentially on the local machine. Capture actual results; do not reuse earlier CI counts.
- [ ] Exercise the spec's six acceptance scenarios on all three platforms. Include browser-import/Link Grabber and ordinary download regressions. Confirm zero out-of-scope requests from deterministic fixture logs and verify offline capture with networking disabled.
- [ ] Record bounded-crawl measurements on the user's 16 GB machine: peak memory, requests, persisted URL count, body bytes and checkpoint duration. A 100,000-URL synthetic graph must honor its configured ceiling without unbounded queues or UI stalls. Fix failing acceptance criteria before closing the gap.
- [ ] Update the feature inventory with the shipped scope and test evidence. Commit with intent `docs: document and verify site grabber delivery`.

## Stop conditions

Planning ends with these documents; no application behavior changes in this task. Implementation ends when all acceptance scenarios pass and limitations are documented. Browser rendering, a public plugin SDK, unrelated protocols, general settings encryption changes, CLI/MCP exposure and the remaining feature gaps require separate scoped work.

If a dependency cannot meet the Rust floor/license requirements, or engine enforcement cannot prevent a forbidden connection before it occurs, resolve that blocker rather than weakening the requirement. Do not hide incomplete tasks behind a feature-complete label.
