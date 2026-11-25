# Full Site Grabber design

Date: 2026-11-24. Status: proposed design for the requested implementation plan; no crawler has been implemented.

## Goal and scope

Allow a Swoopdl user to save a bounded HTTP(S) website or selected files, resume the project after closing the app, browse captured static pages offline, and synchronize the capture later. Cover the supplied prompt's Phase H requirements. Preserve the existing Link Grabber for browser-provided links, pasted lists, and download imports.

“Full” means a complete controlled crawl workflow, not a promise to reproduce every modern web application. The initial release follows links present in HTML/CSS. It does not run JavaScript, submit forms, bypass access controls, or implement a browser rendering engine. JavaScript-only applications and runtime API content must be reported as limitations.

## Approaches

1. **Native Rust crawler using SDL transport: recommended.** Shares network settings, bandwidth accounting, storage conventions, and the egui interface. Requires URL discovery, persistence, and rewriting components.
2. **External mirroring executable.** Quicker initial wrapping, but adds executable distribution, process control, settings translation, and cross-platform dependency management. It would provide a second networking implementation.
3. **Headless browser capture.** Useful for dynamic pages, but adds browser downloads, higher memory use, script execution, and much less predictable crawl behavior. Reserve it for a separate future proposal.

Use approach 1 as the planning assumption. The user requested a plan; this document does not authorize implementation or represent approval of every proposed default.

## User workflow

Tasks → Site Grabber opens a project list. New project asks for a name, start URL, destination, page scope, optional resource origins, filters, and budgets. A review page shows the effective limits before Start. Existing projects offer Resume, Pause, Synchronize, Open offline copy, Report, and Delete project metadata. Deleting captured files requires a distinct explicit action.

```text
Site Grabber
  Project: Product documentation       Paused
  Start URL: https://example.test/docs/
  Scope: same origin, /docs/           Depth: 3
  Saved: 142 files / 83 MiB            Pending: 28
  Skipped: 17                          Failed: 2
  [Resume] [Synchronize] [Open offline copy] [Report]
```

Show reasons such as depth limit, scope, robots, type filter, HTTP failure, and byte limit individually. Network/file work runs off the UI thread. Use existing themes, dialog outcomes, localization, and accessibility patterns. Use Swoopdl/SDL names and existing Swoopdl assets.

## Proposed operating limits

| Setting | Default | Rule |
| --- | --- | --- |
| Page scope | Exact seed origin and seed directory | Explicitly allow additional page origins or a wider path; never infer subdomain permission. |
| Resource scope | Same origin | Additional CDN origins are opt-in and do not authorize page crawling. |
| Navigation depth | 3 | Seed is depth 0; assets do not add navigation depth. |
| Unique URLs | 1,000 | Includes pages, assets, redirects, and rejected discoveries; configurable up to 100,000. Once full, stop admitting discoveries and report the limit. |
| Body-byte budget | 1 GiB per run | Count streamed decoded body bytes, including robots responses, retries and discarded partial responses; retain accounting across pause/restart. No worker may exceed a shared reservation. |
| Single resource | 256 MiB | Enforce while streaming even without Content-Length. |
| Parseable document | 8 MiB decoded | Larger HTML/CSS is reported as unparsed; do not buffer it without a cap. |
| Active requests | 4 total, 2 per origin | One network stream per resource; no segmented crawler downloads. Share SDL bandwidth and host limits. |
| Request pacing | 1 request/second per origin | Includes retries and robots requests; obey a longer Retry-After. |
| Retry policy | 3 attempts; 1/2-second backoff | Retry transient transport failures, 408, 429 and 5xx; do not retry ordinary permanent 4xx. |
| Redirect limit | 10 per request | Check each destination before connecting; record aliases and loops. |
| Connect/read timeout | 30 seconds each | Cancellation interrupts reads and backoff. |
| Robots body | 1 MiB decoded | Separate parser cap within the overall body budget. |

Values are proposed defaults, not measurements. A project that reaches a limit ends as `Limited`, with its reason, rather than claiming a complete capture. Persist a bounded frontier and bounded report records; stream large report exports. Run only one project at a time in the first release.

## Components and existing code

Create a `sdl-grabber` workspace crate for crawl state, policy, discovery, local mapping, rewriting, reports, and synchronization. It must not depend on `sdl-app`. Keep `crates/app/src/grabber.rs` unchanged in purpose; add a separate `site_grabber.rs` interface.

The engine's `http_client` already carries proxy/DNS/adapter settings, but using it alone does not enforce crawl policy. Add an engine-owned bounded fetch adapter using the existing connector and limiter plumbing. It must explicitly control redirects, response streaming, cancellation and aggregate byte reservations. Inspect and reuse the current `scoped_headers` logic internally without exposing a general-purpose unsafe shortcut. Ordinary `Download::start` must retain its existing behavior.

The current private `engine::html` module is deliberately a small helper, not a complete HTML parser. Propose LOL HTML for structural discovery and rewriting, plus a maintained CSS tokenizer for CSS URLs/imports. Resolve exact dependency versions, license notices, parser memory limits, encoding behavior, and Rust compatibility in the first implementation task; pin them in Cargo.lock. Do not parse HTML or CSS with regular expressions.

## URL, request and output rules

- Accept HTTP(S) only. Drop fragments for fetch identity and retain fragments in rewritten links. Preserve query parameter order and values; sorting or dropping them can change resource identity. Do not equate HTTP and HTTPS.
- Resolve relative links against the final response URL and the first valid HTML base URL. Apply scope, filters and budgets after resolution, including redirects. Navigation and asset permissions are separate. Discovered canonical tags do not expand scope.
- Support `a[href]`, frame URLs, stylesheet links, icons, image/source `src` and `srcset`, audio/video sources and posters, CSS `url()` and `@import`, and inline style resources. Capture script files as assets but disable script execution in the offline copy by default. Do not follow forms, refresh loops or JavaScript-created links.
- Filters use explicit URL-prefix include/exclude rules plus MIME/extension allowlists. Excludes win. Page traversal eligibility is separate from which file types the user wants saved; explain that relationship in the wizard.
- Reject URL userinfo. Do not persist raw Authorization/Cookie headers in projects, logs or reports. Default to public unauthenticated requests. Add opt-in origin-scoped session credentials only through verified protected storage or memory-only secrets; unavailable protection must not silently save plaintext.
- Use a crawler-specific identifying User-Agent with product token `SwoopdlGrabber`; do not inherit the engine's compatibility browser identity for crawls. Apply robots rules for each origin using RFC 9309. Use the RFC's matching, redirect, error and caching rules. Treat robots 401/403 conservatively as blocked by product policy. Honor longer server delays; robots permission never constitutes authentication permission.
- Robots redirects are control-plane requests and may need to cross origins under the RFC. Bound them to 10 hops; apply address safety and send no credentials. They do not grant page/resource scope for the redirect origin.
- Block loopback, link-local and private network destinations by default. A user may explicitly authorize a private seed origin for intranet captures; this does not authorize other private origins. Validate resolved addresses at connection time and after every redirect, including IPv6/mapped addresses, without a DNS re-resolution gap. Test fixtures use an explicit injected local policy.
- Generate stable bounded filenames from normalized URL hashes and a MIME-derived safe extension. Use the full SHA-256 key and explicit collision checks; never let Content-Disposition or URL paths choose filesystem traversal. Keep human-readable source mappings in project metadata.
- Write under a project-owned output directory. Reject symlinks/reparse-point escapes, reserved filenames, and paths outside the selected destination. Use temporary files and atomic replacement. Never overwrite unrelated user files.
- Keep immutable raw bodies for rewriting and synchronization. Resolve links through the successful resource map, using relative local paths; preserve fragment suffixes. Links to failed or uncaptured resources remain absolute and appear in the report. Remove the remote base URL after rewriting. Avoid rewriting data URLs and fragment-only references.
- Offline copies disable scripts, event handlers and form submissions by default and apply a restrictive offline policy. Explain that interactive behavior may be absent. Validate the static fixture with networking disabled; external live links remain explicit links rather than silently fetched resources.

## Persistence and project lifecycle

Store project metadata under the SDL data directory, separately from `downloads.json`, using versioned atomic snapshots and backup recovery. Store large raw/output bodies in the selected project destination. Persist the URL frontier, depth, origin policy, URL-to-path map, redirects, validators, hashes, budgets, failure reasons and next synchronization time. Bound metadata by the URL limit.

Lifecycle: `Draft → Running → Pausing → Paused`, then `Running → Completed | Limited | Failed`; cancellation preserves resumable metadata. Only publish an output mapping after its body and metadata are durable. Recover interrupted in-flight entries as pending; account for bytes already consumed. Pause prevents new requests, cancels active streams, and checkpoints before acknowledging Paused. Retry a partial body from the beginning initially; do not claim byte-range resume. A missing destination produces an actionable paused/error state without silently choosing another folder.

Deduplicate content using raw SHA-256 hashes while preserving every URL's mapping and page-specific rewrite base. Identical raw pages at different URLs can resolve relative links differently; never reuse their rewritten output solely by content hash.

Synchronization uses ETag/Last-Modified conditional requests, recrawls changed documents, and rebuilds affected rewrites. A 304 is usable only with a verified saved body; otherwise request the full body. Atomically publish a completed generation; interruption must leave the last usable capture intact. Do not delete prior files simply because a page is temporarily unavailable. Mark missing or changed resources in the report. Offline files are not synchronized back to the server.

Periodic sync uses the app's existing schedule/time handling, one active run, and a persisted next-run marker. No second runtime or autonomous service is required. App-closed schedules run once after reopening when overdue; do not replay every missed occurrence. A manually paused project remains paused until resumed or explicitly re-enabled for scheduled runs.

## Acceptance and delivery boundaries

1. A two-page fixture with nested CSS, imports, images, srcset and a broken link is captured and browsable with networking disabled.
2. No out-of-scope, robots-disallowed, unauthorized private-network, or excluded URL is requested. Redirect and DNS-rebinding cases prove enforcement before connection.
3. Crawl loops, query traps, chunked bodies and parallel downloads cannot escape URL/byte/concurrency limits.
4. Pause, app restart, corrupt-latest-metadata recovery, and destination loss preserve a truthful resumable project.
5. A second sync transfers changed bodies, retains valid unchanged bodies, reports failures, and preserves a usable earlier generation on interruption.
6. Project creation and management work on Windows, Linux and macOS; Link Grabber, regular downloads and browser imports continue to pass their existing tests.

The first usable milestone is a public static-site capture with durable pause/resume and a project UI. The complete Phase H delivery additionally requires offline rewriting, reports, duplicate detection and periodic synchronization. Protected authenticated sessions are a separate final task; do not advertise them before their credential-storage checks pass.

## References

- [Feature inventory](../../FEATURE_GAPS.md).
- [SDL architecture](../../ARCHITECTURE.md).
- [LOL HTML official repository and streaming rewriter examples](https://github.com/cloudflare/lol-html), consulted through Context7 on 2026-11-24.
- [RFC 9309: Robots Exclusion Protocol](https://www.rfc-editor.org/rfc/rfc9309.html), especially sections 2.2–2.5. Product-specific restrictions above are intentionally identified separately.
