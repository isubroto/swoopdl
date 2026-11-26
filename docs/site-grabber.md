# Site Grabber

## Explore and download files

Open **Tasks → Site Grabber**. **Files** is the default workspace; choose **Offline website** for static mirroring. Existing offline projects retain their original format and behavior.

1. Create a project using the four wizard steps: project/start page/session, save location/layout, page exploration rules, and file filters. Automatic downloading defaults off.
2. Choose **Start Exploring**. The scanner reads bounded pages and metadata, then lists candidates without starting full media transfers. Page depth limits HTML traversal; video links on the last permitted page remain candidates.
3. Stop or finish exploration, then check the files to download. Filter the list, sort by size, or select a source folder. Unknown sizes remain unknown and are excluded from known-byte totals.
4. Choose **Download checked** to start the main queue, or **Add to queue** to leave jobs waiting. These jobs use the normal segmented engine and its connection counts, bandwidth controls, disk checks, and range-resume behavior. Stopping exploration does not stop queued downloads.

File mode defaults to depth 3, 1,000 URLs, and a separate 64 MiB exploration budget. Metadata uses HEAD and a Range prefix capped at 16 KiB, including when a server ignores Range. HTML login/error responses are not offered as videos. The mirror's resource/run caps do not become hidden transfer ceilings. Optional file size bounds are rechecked against transfer response metadata; a bounded-size transfer with no verifiable total length fails explicitly.

Choose one folder, original relative subfolders, or category folders. Existing nonempty destinations are supported. Names are checked for traversal, encoded separators, Windows device names, reparse points, and collisions; numbered names preserve existing files. File mode does not publish an offline index. Editing a stopped project changes future scans and undispatched candidates, while dispatched jobs retain their policy and destination.

Selection, exact signed-URL identity, the exploration frontier, and main-list associations are durable. Catalogs live separately from version-1 offline snapshots. Dispatch identities prevent duplicate jobs after repeated clicks or a crash before acknowledgement. Guarded jobs have a separate versioned store that older binaries cannot load as ordinary downloads. Missing guards fail closed, and deleting project metadata does not remove constraints from queued jobs.

Each actual transfer request, segment, retry, redirect, and DNS answer is checked against its immutable capture policy. Robots is verified again for transfers. System routes are resolved per hop; routes requiring proxy-side DNS remain unsupported and are reported without changing settings. Origin-scoped authorization/cookies never follow a new origin. Memory-only sessions must be supplied again after restarting; vault-backed sessions require the vault.

The browser extension adds **Explore page links with Site Grabber**. It imports URLs already rendered in the current page into a new project draft, retaining their source page. Browser cookies remain in memory and are limited to that page's origin. Saving a draft does not start discovery or downloading. Private-window links retain the existing temporary Link Grabber flow. You can also paste exported links under **Import browser links**.

Schedules select explore-only, explore-and-download, checked-files-only, or synchronization. The app must be open. Manually stopped exploration stays stopped; failed runs need manual attention. An overdue schedule is advanced once rather than replaying every missed interval.

HLS/DASH manifests offer **Open media tools** rather than becoming ordinary file transfers. This is an explicit handoff to the existing media workflow. Full browser rendering, script execution, DRM, FTP traversal, proxy-side DNS validation, subdomain expansion, and app-closed scheduling are not provided. Saved custom templates, independent scan/transfer concurrency controls, richer statistics/properties, legacy browser cache behavior, and completion programs/power actions remain later parity work.

## Offline website projects

Choose **Offline website** in **Tasks → Site Grabber** to create a saved static website capture. Link Grabber remains available for importing individual download links.

Give the project a name, an HTTP(S) starting URL, and an empty destination folder. Review page scope, resource origins, exclusions, depth, URL count, and decoded body-byte budget before saving. Additional resource origins permit assets; they do not permit navigation to those origins. URL-prefix exclusions take priority over inclusions. Page traversal and saving selected asset types are separate decisions.

Choose **New project** to open the four-step wizard:

1. **Project & start page:** name the project, enter its URL, and choose a template. The images template selects JPG, PNG, GIF, WEBP and other common image extensions. Authorization is optional and expandable.
2. **Save location:** enter a base folder or choose **Browse…**. A project subfolder is enabled by default. For example, project `skk` under `E:\` saves to `E:\skk`. Review **Files will be saved to** before continuing. Turn the subfolder option off only when you want to use an empty folder directly. **Next** checks the final location before advancing. An optional synchronization schedule is available here.
3. **Explore rules:** choose link depth and whether to stay in the starting folder or explore the exact website origin. Expand advanced rules for additional page origins and URL prefixes.
4. **File filters:** adjust extensions, maximum resource size, URL ceiling and run budget. Expand advanced filters for MIME types and asset origins. Choose **Save project**, then **Start capture** in the project view.

**Back** keeps entered settings. Offline presets configure capture filters. Use Files mode for the separate explore/review/selected-download workflow. Output keeps the existing safe filenames and offline generation layout.

For a failed project with no captured files or charged bytes, use **Change destination** to choose another base folder. The same project settings are retained, and the project returns to Draft for a fresh start. Projects with existing capture data cannot use this repair action. A failed start is shown as **Failed** and does not repeatedly run its schedule; retry manually after correcting the error.

The defaults are depth 3, 1,000 unique URLs, 1 GiB of decoded response bodies per run, a 256 MiB resource cap, and an 8 MiB document parse cap. Robots responses, retries, and discarded partial bodies consume the run budget. Reaching a limit produces **Limited**. Inspect the report before treating any capture as complete.

Requests identify as `SwoopdlGrabber`. Robots exclusions apply to each origin; 401/403 robots responses block capture conservatively. Public captures reject private and reserved addresses. Explicit intranet permission applies only to the exact seed origin. Captures currently require direct routing: configured proxy routes are refused because their remote DNS cannot be verified locally.

Use **Start / Resume** to run a saved project. **Pause** remains pending until active work stops and the checkpoint is durable. Resume restarts an interrupted resource from its beginning. It does not use byte-range resume. Following a crash, an uncertain request reservation is charged conservatively; a run can reach its budget earlier than its successfully saved files suggest. Reconnect a missing destination before resuming.

**Open offline copy** opens the published `index.html`. Each finished capture publishes a separate generation. Raw bodies remain immutable and identical bodies share storage, while pages retain their own URL-based rewrite base. **Export report** produces JSON lines without query values, fragments, authorization headers, or cookies. **Delete metadata** leaves captured output files in place.

**Synchronize** sends conditional requests when a verified saved body exists. A missing or corrupt saved body requires a full fetch. Changed documents are rediscovered, and offline documents are rebuilt from raw bodies. An interrupted synchronization leaves the earlier published generation available. Missing server resources are reported and do not remove previous captured files.

Scheduled runs use the application clock. The application must be open; an overdue project runs once after reopening. A manually paused project stays paused until explicitly resumed. Only one project runs at a time.

Authentication is optional. Supply an authorization header only for an origin you are permitted to access. Memory-only credentials last until the application closes. Persistent credentials require a working system credential vault and a successful read-back verification. A locked or unavailable vault never falls back to plaintext storage. Resume requires the vault to be available or replacement credentials supplied explicitly.

Project metadata is capped at 256 MiB. Each rewritten document is capped at 64 MiB. An oversized checkpoint is rejected before replacing the previous recoverable snapshot. Pause also cancels publication between files; the previous entry point stays available until the new generation is ready.

## Limitations

System proxy settings work when the effective route is verified DIRECT, including per-URL proxy bypass rules. Every redirect gets a fresh route check. Windows automatic discovery now resolves the route for each URL instead of rejecting the discovery flag. A verified DIRECT result or no discoverable WPAD configuration permits the existing direct/static/environment fallback; separately configured proxies are never cleared. A discovered proxy, explicit PAC script, failed discovery, or unknown configuration stops with a routing error. Native discovery runs off the async thread with a request deadline, so pause and cancellation remain responsive. System routing on macOS remains unavailable until automatic settings can be checked independently; explicit direct routing works.

“Robots disallows the destination” means a retrieved policy contains a matching exclusion. “Robots lookup failed” instead identifies a routing, connection, timeout, HTTP, decoding, or local-file problem. Capture stops conservatively and retains the pending URL. After correcting the problem, use **Start / Resume** on the existing project. Failed runs require manual retry and suspend their schedule. Pause and byte-limit stops remain **Paused** and **Limited**, with consumed bytes retained.

For an intentional direct connection, open **Options → Proxy / Socks**, select **No proxy/socks**, and turn off **Use proxy/socks from a browser's request**. This changes network settings for ordinary downloads too; use it only when direct routing is intended. The crawler never changes these settings itself.

Site Grabber follows static HTML and CSS. It does not render JavaScript, automate sign-in forms, submit forms, scrape browser cookies, or reproduce runtime API content. Offline scripts, event handlers, embedded active content, and form submissions are disabled. Interactive features may be absent. Uncaptured hyperlinks remain absolute. The offline content policy prevents automatic external requests.

The first transport uses one active capture request, below the global and per-origin ceilings. Each origin receives at most one request per second, including redirects and retries; longer server retry delays take priority. A redirect to another page origin requires that origin's robots policy to have been checked already; otherwise it is refused.

Cross-platform acceptance, browser verification, and large-graph measurements must be recorded in the implementation plan before claiming the full feature has shipped.

CSS input must use UTF-8. See [verification evidence](site-grabber-verification.md) for completed checks and remaining platform acceptance.
