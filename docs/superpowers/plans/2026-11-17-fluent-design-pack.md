# Fluent design pack implementation plan

Goal: regenerate the existing Rust/egui Fluent presentation from the supplied pack.
Spec: D:/Swoopdl/Swoopdl_Win11_Fluent_Design_Pack/IMPLEMENTATION_PLAN.md

The supplied pack selects the design and implementation approach. Retain the existing theme engine, commands, twelve settings pages, thirteen table column IDs, user column layouts, and settings schema. Fluent remains opt-in; Classic, Native, and contrast themes retain their layouts. No new dependencies or engine changes.

- [x] Align Fluent semantic tokens and comfortable table metrics with the pack.
- [x] Reflow command bar and category rows with matching hit targets and scroll math.
- [x] Add the main content heading and existing Columns action, preserving persisted columns.
- [x] Enlarge Configuration, retain all page painters, and scroll content above a fixed footer.
- [x] Reflow General into setting cards, preserving unsupported controls and all original actions.
- [x] Run formatting, check, Clippy, unit tests, debug build, and native screenshots.

Review focus: RTL coordinates; scrolling at high zoom; all commands still dispatch; all settings controls retained; legacy themes unchanged. Source has no Git metadata, so original files and SHA-256 manifest are saved outside the source tree before edits.

## Validation record

- Read all 14 files in the supplied pack, including six raster images.
- `cargo fmt --all -- --check`: passed.
- `cargo check --locked -p sdl-app --bin swoopdl`: passed before the final review fixes.
- `cargo clippy --locked -p sdl-app --all-targets -- -D warnings`: passed.
- `cargo test --locked -p sdl-app --bin swoopdl`: 320 passed, 0 failed, 1 ignored after review fixes.
- Independent review found a category close hitbox mismatch and missing settings keyboard navigation. Both were corrected. A real egui input test verifies ArrowDown/Home/End page changes and retained focus.
- Native Windows screenshots were captured for the main window and all twelve Configuration pages, light/dark General, Arabic main, and 200% General. Final light/dark screenshots were refreshed from the final debug executable; all twelve pages rendered. Classic Configuration was also captured with its original compact layout.
- Release, Linux/macOS builds, live downloads, 10k-row load, and full assistive-technology end-to-end checks were not run.

Build output: `D:/Swoopdl/.build/fluent-ui/debug/swoopdl.exe`.
Activate using View > Theme > Swoopdl Fluent. Fluent remains opt-in.

## Environment event

An external cleanup removed `SwoopdlSource/target`, the original-file backups, and graph outputs during compilation. The build was restarted with `CARGO_TARGET_DIR=D:/Swoopdl/.build/fluent-ui`. A recovery copy of the final five source/theme files and SHA-256 manifest is stored at `C:/Users/anmon/AppData/Local/Swoopdl-Fluent-20261117`; this is a copy of the edited files, not an original-file rollback backup.

A convenience launcher is available at `D:/Swoopdl/Run-Swoopdl-Fluent.cmd`; it selects Fluent for that launch through `SDL_THEME`, without changing the default theme or the settings schema.
