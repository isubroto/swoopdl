<p align="center"><img src="docs/branding/Swoopdl-wordmark.png" width="430" alt="Swoopdl logo"></p>

# Swoopdl

~**Swoopdl** is an open-source, cross-platform download manager built in Rust.~

**Development version:** see `workspace.package.version` in [Cargo.toml](Cargo.toml). Features include HTTP/HTTPS
transfers, pause/resume, download queues, and torrent support. Capabilities are being validated
on Windows and Linux; the macOS build is experimental.

## Browser extensions

Browser extension files are built from this repository and bundled with Windows, Linux, and
macOS packages. Local setup registers the native messaging host and opens the installed
browser's setup page. No extension download or browser-store visit is required.

On Windows, select browser integration in setup and run the final-page browser setup option.
On Linux, use **Swoopdl Browser Setup** or the bundled shell helper. On macOS, copy the app to
Applications, then run **Set up browser extension.command** from the disk image.

Chromium browsers require **Developer mode → Load unpacked** and user approval. The bundled
Firefox XPI is unsigned and supports temporary development installation; permanent installation
in standard Firefox requires a Mozilla-signed XPI. Safari integration is not included in this setup.
See [local setup instructions](tools/extension/LOCAL-SETUP.txt).

Browser capture is **off by default**. After local setup, enable **Options → General → Use
advanced browser integration** and select the browsers you want Swoopdl to capture from.

## Building

Install the Rust toolchain specified by `Cargo.toml` and build on the target operating system:

```bash
cargo build --workspace --locked
cargo test --workspace --locked
```

The application executable is `swoopdl` (`swoopdl.exe` on Windows).

Building distributable packages also requires PowerShell (`pwsh`) and Node.js for the bundled
extension packer and validator. Use `tools/installer/build.ps1` on Windows,
`tools/installer/build-linux.sh` on Linux, or `tools/installer/build-macos.sh` on macOS,
with each script's native packaging prerequisites. These scripts pack the extension from the
same checkout before creating the installer or archive.

## Branding and third-party credits

The Swoopdl visual identity and logo are under `docs/branding/`.
Swoopdl is an unofficial, independently maintained project based on the source code of
[Mister Download Manager](https://github.com/anefzaoui/misterdownloadmanager)
by **Ahmed Nefzaoui and contributors**. Their original code and design work provide
the foundation for Swoopdl. Swoopdl is not an official release and is not affiliated with
or endorsed by the original project's maintainers. Full authorship, license obligations,
and third-party credits are documented in [ATTRIBUTION.md](ATTRIBUTION.md) and
[docs/UPSTREAM.md](docs/UPSTREAM.md). The original project's releases are **not** Swoopdl releases.

## License

See [LICENSE](LICENSE) and [ATTRIBUTION.md](ATTRIBUTION.md). Preserve all existing copyright
and license notices when redistributing modified source or binaries.
