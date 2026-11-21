# Swoopdl website redesign

Status: approved by the user and implemented on 2026-11-21.

## Intent

Give Swoopdl an original, modern website for a desktop download manager. Replace the inherited visual identity and public-facing references to the earlier product. The user has no connected GitHub repository yet. Do not invent a repository, published release, extension listing, website domain, or download URL.

## Approaches

1. Recommended: a light, spacious product website with an original interface illustration, strong typography, and restrained cobalt and teal accents. This makes the product easy to understand and leaves room for honest release information. These colors match the approved Swoopdl artwork in `docs/branding/`.
2. Alternative: a dark-first technical dashboard aesthetic. This puts more emphasis on technical users and is visually heavier for a general desktop utility.

Both retain a static HTML/CSS/JavaScript website. A framework migration adds no useful capability for this task.

## Visual design

- Background: glacier white `#F8FAFC`; surfaces: white `#FFFFFF`.
- Primary text: slate `#172033`; secondary text: `#526077`.
- Primary brand and links: cobalt `#2563EB`; interaction accent: teal `#0F766E` with a measured accessible foreground.
- Headings and body: self-hosted Plus Jakarta Sans, with system fallbacks. Use large sentence-case headlines, comfortable reading width, and tabular numerals for file data.
- Use the approved Swoopdl mark from `docs/branding/Swoopdl-icon-1024.png`. Generate website favicon and social assets from that mark. Do not reuse inherited logos, favicons, social cards, screenshots, or browser artwork from the earlier website.
- Desktop hero: left-aligned headline and explanation next to a large original download-list illustration. Label the illustration as an interface concept, not a screenshot of the shipping application.
- Mobile: headline and action first, illustration second. Avoid tilted labels, overlap, postage fields, and barcodes.
- Use whitespace, purposeful borders, and a limited range of corner radii. No looping decorative motion, invented statistics, testimonials, or benchmark multipliers.
- Light and dark themes use the same information hierarchy and accessible contrast.

## Homepage structure

```text
Swoopdl       Features   Setup   Help        Theme   Get Swoopdl

Big downloads.                    Original Swoopdl
Small effort.                     interface concept
Plain product explanation         with sample downloads
[Get Swoopdl] [Explore features]
Honest development/release status

HTTP downloads    Torrents    Video links

Feature walkthrough with user-controlled tabs
Browser setup with clear local-install instructions
Windows / Linux / macOS availability
Short FAQ
Final action and simple footer
```

Copy may be refined during implementation. Every product claim must be supported by the current source. Browser integration is an explicit local setup flow; Firefox signing limitations and capture being off by default must remain clear.

## Website scope

- Redesign homepage, download/setup page, shared navigation/footer, 404 page, and generated feature/help/legal page presentation.
- Replace the comparison and benchmark landing pages with useful Swoopdl information or redirects to current destinations. Do not publish inherited competitor comparisons, benchmark claims, or screenshots.
- Audit generated HTML, metadata, social previews, favicons, manifests, scripts, translation output, and copied public assets for inherited branding and links.
- Do not modify the Rust application or its artwork. Keep any required third-party legal notices in their proper legal context; do not turn them into promotional content.
- Curate or sanitize inherited help content before publishing it. Do not publish a screenshot that still displays the earlier product name.
- Preserve useful URLs where practical through redirects or refreshed content. Do not leave navigation pointing to removed pages.
- Do not publish stale translations that restore removed branding. Retain the localization mechanism where feasible; advertise only translations that are ready for the new copy.

## Release and configuration behavior

- A single site configuration controls optional repository, public URL, release metadata, and download assets.
- With no configured repository or release assets, the primary action leads to an availability/setup page that says public downloads are not available yet. Never make a button pretend to download.
- Do not query upstream release APIs, sponsor pages, contributor lists, extension stores, or analytics accounts.
- Omit production canonical URLs, CNAME, release download schema, and external project links until real values are configured. Local preview works without GitHub credentials or network calls.
- Version labels distinguish the source development version from a published downloadable version.
- Remove the inherited analytics integration. Self-host fonts and assets.

## Accessibility and interactions

- Semantic headings, a skip link, visible keyboard focus, descriptive button labels, and keyboard-accessible menus and tabs.
- Main controls have at least 44px touch targets. No horizontal overflow at 320px and above.
- Normal text contrast at least 4.5:1. Do not rely on color alone for state.
- Remember theme preference. Respect reduced motion. Feature walkthrough changes only through user actions.
- The website works for reading and navigation without JavaScript.

## Verification and completion

- Build the actual site with no upstream network dependency.
- Verify JavaScript syntax and all generated internal links and assets.
- Scan generated output for inherited product names, upstream domains, store IDs, old screenshots, and analytics identifiers.
- Inspect desktop and mobile screenshots in both themes.
- Exercise menus, theme persistence, feature controls, FAQ, and download availability behavior in a browser.
- Keep all source changes local. Do not commit, publish, deploy, or connect an account.

## Builder decisions

The builder may settle precise spacing, icon paths, responsive breakpoints, secondary page layouts, module boundaries, and copy details within this design. Existing Python build tooling may be adapted or simplified to ensure clean, offline output. Dependencies should remain minimal. No additional user preference is required beyond approval of this visual direction and scope.
