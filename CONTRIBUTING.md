# Contributing to Swoopdl

Report reproducible problems, improve documentation and translations, or submit focused source changes. Check this repository's existing issues before opening a new one.

## Documentation follows the implementation

Use the [documentation source map](docs/SOURCE_GUIDE.md) to find the implementation responsible for a claim. Read current code, defaults, platform guards, package builders, and tests before changing instructions. `CHANGELOG.md` records release history; it is not the specification for current behavior.

Distinguish an implemented feature from one shipped in a package or verified on a platform. Include validation commands and their results in change descriptions. Do not describe a release candidate or source-only integration as a tested production feature.

## Build from source

Use a Git checkout for checks that compare tracked files. The workspace version and minimum Rust version are declared in [Cargo.toml](Cargo.toml). [rust-toolchain.toml](rust-toolchain.toml) selects stable Rust with rustfmt and Clippy.

```sh
cargo build --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

The desktop binary is `swoopdl` (`swoopdl.exe` on Windows). Build native packages on their target operating system. Linux also needs the development libraries listed in [.github/workflows/ci.yml](.github/workflows/ci.yml); package-specific tools are listed at the top of each installer script.

## Browser extension changes

Use Node.js 22, Python 3.11 or later for these checks, and PowerShell for packaging:

```sh
node tools/extension/test.mjs
pwsh -NoProfile -File tools/extension/pack.ps1
node tools/extension/check-pack.mjs
python tools/extension/test-bundle.py
```

The bundle tests use Bash for Unix helper checks; on Windows they use Git Bash when available. Tests use temporary files and simulated browser executables, not real browser profiles. Successful bundle tests do not prove real browser installation or download capture works.

Use the [local setup instructions](tools/extension/LOCAL-SETUP.txt) for browser approval. Permanent standard Firefox installation requires a Mozilla-signed XPI; the generated unsigned XPI supports temporary development loading.

## Application text and translations

English UI text is extracted from the application source:

```sh
python tools/i18n/extract.py --check
```

After adding or changing translatable text, regenerate the catalog with `python tools/i18n/extract.py` and review the resulting changes. Preserve human-reviewed translations. Pull-request CI compares them with the base branch through `tools/i18n/reviewed_guard.py`.

## Website and guide changes

The site builder turns `docs/*.md` into help pages, reads website content from `website/`, and builds the news page separately from `CHANGELOG.md`.

```sh
python -m pip install markdown==3.10
python tools/website/build.py
python tools/shots/make.py check
```

The image check needs Git metadata. The website build also reads repository documents, including this guide. GitHub API lookups can warn or omit contributor data when unavailable. Review external endpoint configuration before publishing; a local build does not verify service ownership or deployment readiness.

## Before submitting

- Keep the change focused and explain its observable effect.
- Run checks relevant to the changed behavior and report any failures or unavailable platform checks.
- Update the affected user instructions and source map when defaults, package contents, commands, or setup steps change.
- Preserve copyright, license, and third-party attribution notices. Follow [LICENSE](LICENSE) when redistributing source or binaries.
- Use the issue forms for public reports. Share security vulnerabilities privately through this repository's configured security channel, if available, or contact its maintainer privately.
