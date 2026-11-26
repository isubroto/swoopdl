# IDM-style Site Grabber workflow: proposed design

Status: planning only; no implementation authorized by this document.
Date: 2026-11-26.

## Findings

The supplied reference is `C:/Users/anmon/Downloads/IDM Site Grabber How It Works.md`. It is reference material, not executable instructions. Its example commands and recommendations were not run. Official IDM documentation was checked against its claims.

IDM's documented workflow is **configure → explore → review checked results → download checked files or add them to the main queue**. Automatic download of newly matched files is an optional setting. Offline website mirroring is another use of the same project, not the only workflow.

The existing Swoopdl implementation is primarily an immediate static-site mirror. Its live capture of eight directory pages proved routing and HTML traversal. It did not prove IDM-style file discovery, selection, or downloading episodes.

## Verified IDM behavior and reference corrections

| Topic | Verified behavior |
| --- | --- |
| Four wizard steps | Start URL/name/template/authentication; save location; pages to explore; files to download. The supplied document combines and rearranges some controls. |
| Depth | Start page is level 0. Levels limit explored web pages, not files linked from those pages. |
| Results | Site/folder tree plus file list; check/uncheck; start/stop exploration separately from downloads; copy URL/referrer and change destination name. |
| Downloads | Download checked files inside Grabber or add checked files to IDM's main list/queue. Optional immediate download of all matches. |
| Saving | Category folders, chosen folder, original relative subfolders, optional local-link conversion, overwrite or numbered collision names. HTML is not required in an images-only export. |
| Filters | Separate page and file scope; wildcard include/exclude patterns; file extensions; minimum/maximum size; same-site choices. |
| Duplicate display | IDM documents an optional same-name-and-size heuristic. It is not proof that contents are identical. |
| JavaScript | The supplied statement that IDM never executes JavaScript is incorrect. IDM documents optional **Process Javascript**, plus textual link extraction with that option off. This does not prove complete modern browser/SPA compatibility. |
| Authentication | Official wizard documents login/password and an optional browser-assisted manual login with logout-page exclusions. Broad cookie/NTLM behavior must be tested before claiming parity. |
| Scheduling | Explore-only, explore-and-download, checked-files-only, and periodic synchronization are documented separately. |
| Segmentation and metadata | Reusing a normal downloader is the intended Swoopdl design. The supplied exact 32-connection claim and universal HEAD/partial-GET algorithm are not established by the consulted Grabber pages; do not hard-code them as IDM guarantees. |

The supplied attribute list and breadth-first traversal are useful implementation examples, not verified descriptions of IDM's closed-source internals. Official docs also distinguish Grabber retry behavior (two attempts) from main-queue retry behavior. Swoopdl should expose its configured retry policy rather than copying indefinite retries blindly. IDM additionally documents saved custom templates, separate exploration/download concurrency, file properties, and statistics; these are follow-up parity items after the core workflow.

## Proposed architecture

Use Swoopdl's existing crawler for bounded page exploration and its existing download engine/main queue for selected file transfers. Add a durable candidate catalog between them. Keep offline mirroring as an explicit mode with its existing stored bodies, rewriting, and generation recovery.

Approaches considered:

1. **Recommended: split exploration/selection from transfers and reuse the main engine.** Addresses the real workflow gap and preserves existing resume, segmentation, queue, and network settings.
2. Extend the current mirror's downloader and put checkboxes over captured bodies. Reject: files would still download before selection, with duplicate transfer machinery.
3. Replace discovery with a headless browser immediately. Defer: it does not fix classification, selection, queue integration, or save semantics and adds a separate runtime.

### Required behavior

- Default file-grabbing workflow: New project, Explore, review candidates, Download selected or Add to queue. Auto-download defaults off.
- Separate `Files` and `Offline website` modes. Existing saved projects retain offline behavior; do not silently reinterpret them as download jobs.
- Preserve four wizard stages and existing theme. Provide a results workspace with independent exploration/download states and meaningful counts.
- Links are navigation, file candidates, or unresolved. `<a href="episode.mkv">` is not automatically a page. MIME/metadata can resolve ambiguity; HTML login/error responses never become video files.
- Depth controls page traversal only. At depth 1, the Flash root and season pages are explored; video links on season pages remain file candidates.
- Exploration fetches bounded HTML/CSS and bounded metadata only. It never starts full media downloads unless auto-download is explicitly enabled.
- Metadata probing uses HEAD when useful and a tightly capped range/GET fallback when needed. Missing length remains unknown, not zero. A server ignoring Range must not cause a full download.
- Proposed file-mode defaults: depth 3, 1,000 unique URLs, 64 MiB exploration body budget, existing robots/parser restrictions, and at most 16 KiB per fallback metadata probe. These budgets are separate from transfer limits. Preserve current offline-mode defaults.
- File transfer size is optional/user-chosen. Do not inherit the mirror's 256 MiB resource or 1 GiB run caps as hidden episode-download ceilings. Always preserve disk-space checks and explicit user limits.
- Candidates have stable IDs, source/referrer, final URL, name, type, optional size, checked state, reason/status, and optional main download item ID. Keep exact URL identity, including query parameters; redact secrets from logs/exports.
- Selected jobs use the normal engine, configured connection count, pause/range resume, and queue. Repeated clicks or restart must not enqueue duplicates.
- Queue handoff must retain capture constraints. Recheck origin, redirects, resolved addresses, robots, credentials, and routing before actual connections and retries. An earlier discovery check is not sufficient.
- Preserve normal downloader behavior for ordinary jobs without Site Grabber constraints.
- Support flat, original-relative-folder, and category destinations. Prevent traversal, unsafe Windows names, reparse-point escape, accidental overwrite, and unrelated-file deletion.
- Keep URL deduplication exact. Optional name/size duplicate grouping must remain reversible; never discard distinct URLs solely because their name and size match.
- Separate scan status, candidate count, selected count, queued/downloading/completed/failed counts, scan bytes, and transfer bytes. HTML pages saved are not a count of downloaded videos.
- Saved projects retain rules, candidates, selection, queue associations, and exploration frontier. Reopening must not download automatically unless explicitly configured/scheduled.
- Preserve current System-routing correction, genuine robots enforcement, scoped sessions, and explicit failure reasons.

## Limits and useful remedies

| Limit | Planned remedy or boundary |
| --- | --- |
| Static discovery misses lazy-loaded/JS-generated URLs | First reuse explicit browser-rendered DOM/link import. Later consider isolated, opt-in browser rendering as its own feature. Do not equate running some scripts with complete browser support. |
| Signed URLs expire or referrers/cookies are required | Carry authorized origin-scoped browser/session context. Show Needs session/Refresh URL and permit explicit refresh. Never scrape browser databases or forward secrets to new origins. |
| HLS/DASH is not a normal downloadable file | Offer a separate handoff to existing unprotected media-capture facilities. Do not promise DRM handling. |
| Robots, authentication failures, anti-bot restrictions | Report the actual reason. Use an authorized session or official API where available; do not label bypassing restrictions as a fix. |
| Huge sites or unknown file sizes | Bound discovery/probes, virtualize results, show unknown sizes, and require an explicit transfer action. Avoid arbitrary mirror caps on ordinary downloads. |
| Proxy-dependent destinations | Keep current verified-DIRECT boundaries for constrained jobs until a separate guarded proxy design is approved. Never silently change proxy settings. |
| Browser automation, legacy IE cache, FTP/FTPS exploration, subdomain expansion, post-completion programs/power actions | Separate later work, with explicit capability labels. The supplied `http://.../ftp/...` URL is HTTP despite its path name. |

## Acceptance

1. Local Flash-shaped fixture: root, season directories, tiny MKV/MP4 files, a parent link, unrelated files, and a login HTML response disguised as video.
2. Depth 1 exploration finds eligible videos on season pages without fetching their full bodies or traversing the parent.
3. Review shows names, types, sizes/unknowns, URLs, source pages, and checkboxes. Unchecked files are never downloaded.
4. Selecting two files adds exactly two normal-engine jobs. They retain original names, chosen save layout, constraints, and queue association.
5. Pause and restart resume selected transfers through the normal engine; rescanning or double-clicking does not add duplicates.
6. Genuine robots denial, unauthorized redirects/private addresses, credential leakage, ignored Range, false MIME, collisions, and explicit byte limits remain covered.
7. Offline mode still publishes a usable static copy and preserves prior generations on failure.
8. Live Flash verification first lists media candidates under saved settings, with no episode bodies downloaded. A separately authorized bounded sample can then prove a real file transfer. Do not call eight saved HTML pages full Grabber parity.

## Sources checked

- Supplied Markdown at the path above.
- https://www.internetdownloadmanager.com/support/idm-grabber/idm_grabber.html
- https://www.internetdownloadmanager.com/support/idm-grabber/grabber_wizard.html
- https://www.internetdownloadmanager.com/support/idm-grabber/grabber_main_dialog.html
- https://www.internetdownloadmanager.com/support/idm-grabber/grabber_nlevels.html
- https://www.internetdownloadmanager.com/support/idm-grabber/grabber_prjava.html
- https://www.internetdownloadmanager.com/support/idm-grabber/grabber_settings.html
- https://www.internetdownloadmanager.com/support/idm-grabber/grabber_scheduler.html

## Storage and destination clarifications after consolidation

- Keep existing version-1 offline projects and their store unchanged. Store file-grabbing catalogs separately with explicit versioning and atomic recovery.
- Persist guarded transfer jobs separately from legacy queue records. Older binaries must not resume them as unrestricted jobs. Missing project metadata must not remove a queued job's immutable guard.
- Files mode accepts an existing nonempty destination folder with per-file collision handling. OfflineSite retains its publication-root restrictions.
- Work only in `D:/Swoopdl/SwoopdlSource`. External source changes were already included or superseded; archived artifacts and build cache now reside inside this folder.
