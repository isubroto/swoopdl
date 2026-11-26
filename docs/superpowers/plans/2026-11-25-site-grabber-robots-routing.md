# Site Grabber robots and default routing repair

> Planning only. Use `executing-plans` for implementation after the user accepts this plan. Do not delegate, switch the user's checkout, discard pending changes, change global proxy settings, publish a release, or disable robots enforcement.

**Goal:** Capture permitted directory listings with the application's default network settings and explain lookup failures accurately.

**Architecture:** Resolve the effective crawl route for each request/redirect before constructing the checked client. Represent robots lookup failures separately from parsed rules. Preserve existing address checks, budgets, credential boundaries, and publication behavior.

**Tech stack:** Existing Rust workspace, reqwest 0.13.5, Tokio, existing proxy matcher, `sdl-engine`, `sdl-grabber`, and the native application. No new dependency is expected.

**Source basis:** `origin/main` at `9645726f13aef93837f71013f6eec266be6da596`, version 0.1.7. The current working checkout is the older `main` at `edad5e5`, version 0.1.6, without the crawler files. Inspection used `git show`; the working tree was not switched. Existing website translation edits and the untracked feature-gap document must remain intact. Original design authority: `origin/main:docs/superpowers/specs/2026-11-24-full-site-grabber-design.md`.

## Evidence collected on 2026-11-25

| Check | Result |
| --- | --- |
| GET `http://203.76.96.50/robots.txt` with `SwoopdlGrabber/0.1` | HTTP 200, 67 body bytes, `User-agent: *` with an empty `Disallow:`. |
| Same endpoint with a browser User-Agent | HTTP 200 and the same rules. |
| Robots request with `Accept-Encoding: gzip` and with `identity` | Both returned the same valid plain-text body; no compression mismatch was observed. |
| Bounded GET of the supplied start page with both User-Agents | HTTP 200, an 876-byte HTML directory listing with `s01/` through `s07/` links. No media files were downloaded. |
| Actual robots parser from the inspected 0.1.7 source, compiled as a standalone probe | Allows `/ftp/TVseries/english/The%20Flash/` for the live body. `Robots::blocked()` rejects the same path. |

Raw probe results and source snapshots are under `.superpowers/sdd/2026-11-25-grabber-robots-plan/`. Server responses are point-in-time observations. The user's effective runtime proxy/DNS/adapter settings were not captured; reproduce that configuration before claiming its exact transport error is proven. No IDM process was run, so the User-Agent comparison is not an IDM implementation comparison.

## Confirmed defects and causal path

1. `Settings::proxy()` defaults to `Proxy::System` because browser/system proxy inheritance is enabled. The existing `connection_exceptions_and_proxy` test explicitly asserts this. `App::request_parts()` passes that selection into Site Grabber.
2. `crates/engine/src/crawl_fetch.rs:584` rejects every mode except `Proxy::None`, before sending any request. It therefore rejects the normal system-proxy selection even when the effective route is direct.
3. `crates/grabber/src/crawl.rs:300` starts a lookup with `Robots::blocked()`. Its `Err(_)` branch around line 349 discards the real fetch error, retries, and then caches that deny-all object. `ScopedAccess::check_url()` around line 105 reports this as `Robots disallows the destination` and the coordinator removes the pending URL.
4. This default-settings path explains a failure before reading response bytes and can produce the supplied report even though the server permits the path. The live parser is not the demonstrated fault.

## Required behavior

- `Proxy::None` continues to use the existing checked direct transport.
- `Proxy::System` with a verified effective DIRECT result must work. Resolve it with semantics consistent with reqwest and the existing system matcher, including per-URL bypass rules.
- Keep the original routing selection and reevaluate it for every redirect. Do not normalize the whole project to direct after checking only the seed.
- Once DIRECT is established for a hop, build that hop's client with automatic proxy discovery disabled. Otherwise a later automatic proxy decision could bypass checked DNS/address enforcement.
- An actual configured proxy, unsupported automatic configuration, or failed route resolution must stop with an actionable routing error. Do not silently connect directly around a configured proxy. Manual/PAC/SOCKS proxy support is outside this repair.
- A parsed robots rule denial must remain a denial, with zero requests to the denied page.
- Failed robots retrieval must remain conservative, but its reason must identify routing, DNS/connect, timeout, HTTP status, decoding, or local I/O as appropriate. It must not pretend that a `Disallow` rule was read.
- Retain the pending seed on a retryable lookup failure. A corrected configuration followed by Start/Resume must be able to retry without deleting and rebuilding the project.
- Pause/cancel and quota exhaustion must remain Paused/Limited outcomes with durable accounting, rather than being turned into robots denials.
- Preserve the existing deliberate policy that robots HTTP 401/403 stops capture. Preserve current 404/unavailable handling, bounded retries, Retry-After, redirect controls, and real disallow rules unless a separately documented requirement changes them.

## Task 1 — Reproduce default settings and preserve the real error

**Files:** `crates/app/src/settings.rs` tests; `crates/grabber/tests/acceptance.rs`; `crates/engine/src/crawl_fetch.rs` tests.

- [ ] Add failing tests using the real default Settings proxy selection. Do not substitute `Options::default()` with its different `Proxy::None` default.
- [ ] Use an injected deterministic effective-route resolver/test seam. Do not mutate global proxy environment variables or Windows settings in tests.
- [ ] Add a regression where a routing refusal during `/robots.txt` yields its routing cause, not `Robots disallows the destination`.
- [ ] Add a fixture with the exact empty-Disallow body and an encoded-space directory path. Assert it is allowed by the parser and reaches the listing under a direct route.
- [ ] Record the observed underlying error for the user's configuration with sanitized diagnostics. Keep runtime-settings inference separate from the confirmed default-settings defect.

Expected initial result: default system-mode capture fails before the seed request; the old coordinator reports the wrong reason.

## Task 2 — Resolve system-mode DIRECT safely

**Files:** `crates/engine/src/crawl_fetch.rs`; `crates/engine/src/net.rs` only if the existing resolver contract is insufficient; `crates/grabber/tests/transport.rs`.

- [ ] Inspect `net::system_proxy()` and reqwest's matcher/feature behavior for the pinned versions. Its `Option` currently conflates some lookup/parsing paths; prove when None establishes DIRECT. Add a crawl-specific fallible resolver if unsupported/unknown configuration cannot otherwise be distinguished. Keep ordinary downloads' contract unchanged.
- [ ] Implement the smallest per-hop route decision helper. Apply a DIRECT decision only to a local clone used to construct that hop's client.
- [ ] Test explicit direct mode, system DIRECT, system proxy configured, resolution failure, and a per-host bypass/direct decision.
- [ ] Test a redirect whose first hop is DIRECT but whose next hop requires a proxy. Assert refusal before connecting to the second destination.
- [ ] Rerun DNS rebinding, private-address, credential-isolation, adapter-binding, and bandwidth/budget tests. All original restrictions must still hold.

Acceptance: default system mode with no effective proxy reaches the fixture; a configured proxy is not bypassed.

## Task 3 — Separate robots policy from lookup state

**Files:** `crates/grabber/src/robots.rs`; `crates/grabber/src/crawl.rs`; `crates/grabber/tests/robots.rs`; `crates/grabber/tests/acceptance.rs`; `crates/grabber/tests/recovery.rs`.

- [ ] Replace the synthetic deny-all fallback for unsuccessful retrieval with a small explicit lookup outcome/error representation. Cache parsed/resolved policy, not a fabricated rule set after a transient failure.
- [ ] Retain the seed/frontier entry on lookup failure and surface the terminal reason. Retry transient failures within the existing limits; do not repeat a permanent unsupported-routing configuration error three times.
- [ ] Preserve genuine group selection, empty rules, longest-match/Allow ties, percent encoding, and actual Disallow enforcement.
- [ ] Add tests for 200 empty Disallow, 200 real Disallow, 404, conservative 401/403, 503/Retry-After, DNS/connect failure, timeout, decoding failure, and local-file failure.
- [ ] Add pause during robots fetch/backoff, robots/body cap exhaustion, and resume-after-configuration-correction tests. Verify pending entries and charged bytes survive correctly.
- [ ] Assert a genuine disallow never reaches the protected page. Assert a lookup failure never produces the rule-denial message.

Acceptance: the same path can be allowed, denied by a real rule, or blocked because lookup failed, and the report accurately distinguishes those cases.

## Task 4 — Make the failure actionable in the app and report

**Files:** `crates/app/src/site_grabber.rs`; `crates/app/src/site_grabber/view.rs`; `crates/grabber/src/report.rs` if required; generated `i18n/en.tsv`.

- [ ] Carry the reason through worker result, progress events, selected-project error, and JSON report. Do not lose the cause at an error wrapper or overwrite it with a generic denial.
- [ ] Report safe error categories and HTTP status without raw Debug output, proxy credentials, authorization/cookie headers, or unredacted query strings.
- [ ] Preserve the 0.1.7 Failed/manual-retry behavior and schedule suspension. Keep budget stops Limited and explicit pauses Paused.
- [ ] Add an app-level default-settings capture test and report-redaction test, including a small report/URL limit where the useful cause must not be displaced by a generic second error.
- [ ] Regenerate the English catalog. Check website primary-language coverage for any new published copy; retain the existing threshold and prior translation edits.

Acceptance: a user can distinguish a real robots rule from a network configuration problem and retry after correction.

## Task 5 — Verify the reported flow without large downloads

**Files:** existing labserver fixtures; `docs/site-grabber.md`; `docs/site-grabber-verification.md`; `CHANGELOG.md`.

- [ ] Run a local directory-listing fixture with the app's default settings, empty Disallow, season links, and tiny matching video payloads. Verify normal discovery, scope boundaries, nonzero saved resources, and exact charged bytes.
- [ ] Optionally repeat the already bounded live robots/listing probe using the repaired engine. Do not download full episodes as an automatic test, and do not use a browser User-Agent to conceal the crawler.
- [ ] Run focused engine/grabber/app tests, then fmt, translation checks, website tests, workspace Clippy and the Windows workspace suite. Use `CARGO_BUILD_JOBS=1`, builds sequentially, and an isolated cache if needed. Record exact results.
- [ ] Document effective DIRECT support, remaining proxy limits, distinct robots failure reasons, and how to retry an affected saved project. Update the changelog only with implemented and verified changes; do not claim this repair has shipped during planning.

Implementation workspace: start from the inspected 0.1.7 source in an isolated worktree/branch so the current older checkout and pending website edits stay intact. No dependency upgrade, app-version bump, schema migration, installer publication, or unrelated crawler redesign is part of this plan unless separately requested.

## Done conditions

1. Default system-proxy selection works when its effective route is DIRECT.
2. The supplied robots body permits the encoded-space seed path.
3. Fetch/configuration failures never masquerade as robots rules; the seed remains retryable.
4. Actual proxy routing is not bypassed; DNS, address, credential, scope, and byte limits remain enforced.
5. A true Disallow still prevents the destination request.
6. The local app-to-engine directory capture passes, along with required regression gates and redaction checks.

## References

- [Observed robots endpoint](http://203.76.96.50/robots.txt).
- [RFC 9309](https://www.rfc-editor.org/rfc/rfc9309.html), especially empty rules, access results, and unreachable handling in sections 2.2.2 and 2.3.1.
- [Reqwest 0.13.5](https://github.com/seanmonstar/reqwest/tree/v0.13.5) and the inspected shared client builder/system matcher. Existing source explicitly disables automatic discovery for Proxy::None; implementation must retain that property for verified DIRECT hops.
