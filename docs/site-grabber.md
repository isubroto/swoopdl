# Site Grabber

Open **Tasks → Site Grabber** to create a saved static website capture. Link Grabber remains available for importing individual download links.

Give the project a name, an HTTP(S) starting URL, and an empty destination folder. Review page scope, resource origins, exclusions, depth, URL count, and decoded body-byte budget before saving. Additional resource origins permit assets; they do not permit navigation to those origins. URL-prefix exclusions take priority over inclusions. Page traversal and saving selected asset types are separate decisions.

Choose **New project** to open the four-step wizard:

1. **Project & start page:** name the project, enter its URL, and choose a template. The images template selects JPG, PNG, GIF, WEBP and other common image extensions. Authorization is optional and expandable.
2. **Save location:** enter a base folder or choose **Browse…**. A project subfolder is enabled by default. For example, project `skk` under `E:\` saves to `E:\skk`. Review **Files will be saved to** before continuing. Turn the subfolder option off only when you want to use an empty folder directly. **Next** checks the final location before advancing. An optional synchronization schedule is available here.
3. **Explore rules:** choose link depth and whether to stay in the starting folder or explore the exact website origin. Expand advanced rules for additional page origins and URL prefixes.
4. **File filters:** adjust extensions, maximum resource size, URL ceiling and run budget. Expand advanced filters for MIME types and asset origins. Choose **Save project**, then **Start capture** in the project view.

**Back** keeps entered settings. Presets configure real capture filters; they do not add JavaScript rendering or a separate scan-only download queue. Output keeps the existing safe filenames and offline generation layout.

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

Site Grabber follows static HTML and CSS. It does not render JavaScript, automate sign-in forms, submit forms, scrape browser cookies, or reproduce runtime API content. Offline scripts, event handlers, embedded active content, and form submissions are disabled. Interactive features may be absent. Uncaptured hyperlinks remain absolute. The offline content policy prevents automatic external requests.

The first transport uses one active capture request, below the global and per-origin ceilings. Each origin receives at most one request per second, including redirects and retries; longer server retry delays take priority. A redirect to another page origin requires that origin's robots policy to have been checked already; otherwise it is refused.

Cross-platform acceptance, browser verification, and large-graph measurements must be recorded in the implementation plan before claiming the full feature has shipped.

CSS input must use UTF-8. See [verification evidence](site-grabber-verification.md) for completed checks and remaining platform acceptance.
