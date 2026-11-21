# Documentation source map

This page records where current behavior is implemented. Use it to check documentation against the checkout being documented. Source implementation, packaged availability, and a successful runtime test are separate claims.

## Which evidence to use

1. Read the relevant implementation, defaults, platform guards, and build scripts.
2. Check the tests that exercise the behavior. Record the command and result when claiming a test passed.
3. Describe only the behavior available in the relevant build or package. Source code for a feature does not prove it is shipped or enabled.
4. Use `CHANGELOG.md` for release history. A release-candidate entry does not override the version in `Cargo.toml` or prove a platform was tested.

## Current behavior and its source

| Subject | Implementation to inspect | Documentation consequence |
|---|---|---|
| Version and Rust requirements | [Workspace manifest](../Cargo.toml), [toolchain](../rust-toolchain.toml) | Read `workspace.package.version` and `rust-version`; do not infer them from the newest changelog heading. |
| Supported transfer schemes | [Engine download dispatch](../crates/engine/src/download.rs), [FTP implementation](../crates/engine/src/ftp.rs) | `supported` accepts HTTP/HTTPS and the FTP schemes `ftp`, `ftps`, and `ftpes`. Runtime success still depends on the server and build. |
| App defaults | [Settings](../crates/app/src/settings.rs) | `Settings::default` disables `advanced_integration` and sets `extension_never_ask` to true. Browser setup alone does not enable capture in the app. |
| Browser capture decisions | [Shared capture rules](../crates/ipc/src/capture.rs) | Capture can be declined when integration is off, a type or site is excluded, or other rules reject it. Document both native messaging setup and app settings. |
| Automatic extension prompt | [Extension window](../crates/app/src/extension.rs) | `worth_asking` returns false. Do not promise a browser-installation window on every startup. |
| Local extension files | [Packer](../tools/extension/pack.ps1), [package checks](../tools/extension/check-pack.mjs) | The packer creates `target/extension/bundle` and `sdl-local.zip` from this checkout. Generated store-oriented files do not imply store publication. |
| Windows setup | [Package builder](../tools/installer/build.ps1), [installer](../tools/installer/sdl.iss), [local helper](../tools/extension/install-local.ps1) | The browser task includes the local bundle. The helper selects a detected browser and requires browser approval. |
| Linux setup | [Package builder](../tools/installer/build-linux.sh), [local helper](../tools/extension/install-local.sh) | Packages and extracted archives include the bundle. AppImage setup requires a permanent extraction path. |
| macOS setup | [Package builder](../tools/installer/build-macos.sh), [launcher](../tools/extension/Set%20up%20browser%20extension.command) | Chromium/Firefox files are copied into app Resources. Copy the app to Applications before setup. This builder does not bundle a Safari app extension. |
| Portable diagnostics | [Extension checks](../crates/app/src/extension.rs), [portable data selection](../crates/ipc/src/lib.rs) | Manual registration writes user entries, but `checks` still flags portable mode. Do not promise the same diagnostic/support status as an installed copy. |
| Help links and update feeds | [Help implementation](../crates/app/src/help.rs) | Repository, website, and support links use optional build-time values. Automatic update checks require a configured feed; a development build may have none. |
| Command-line entry | [CLI launcher](../crates/cli/src/main.rs), [commands](../crates/cli/src/lib.rs) | `swoopdl-cli` launches the companion app with CLI arguments. Document commands from the parser and current help. |
| MCP entry | [MCP executable](../crates/mcp/src/main.rs) | The executable serves MCP over standard input/output and forwards requests to the app. Presence does not prove any particular assistant configuration was tested. |
| Website content | [Site builder](../tools/website/build.py) | `build_help` reads `docs/*.md`; `build_news` separately reads `CHANGELOG.md`. User documentation is not generated from changelog entries. |
| CI and publication | [CI](../.github/workflows/ci.yml), [release workflow](../.github/workflows/release.yml) | Read current triggers, conditions, and required jobs. A workflow definition alone does not establish a successful release. |

## Checks for documentation changes

- Confirm every path, command, executable name, and setting against its implementation.
- Check defaults separately from available options.
- Keep platform-specific requirements and unsigned Firefox limitations visible.
- Preserve working section anchors and keep screenshot captions aligned with `tools/shots/shots.toml`.
- Describe known failures or untested paths instead of presenting source inspection as runtime validation.
- Check site configuration before publishing. The website builder currently contains legacy external URLs; those values must not be assumed to identify an owned Swoopdl service.

See [Contributing](../CONTRIBUTING.md) for build and validation commands.
