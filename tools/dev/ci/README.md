# Local workflow checks

Run from a Git checkout before pushing. Native checks read the current working tree,
including uncommitted changes. They do not install a Git hook or push anything.

## Windows setup

Prerequisites: Rustup with MSVC C++ build tools and CMake, Python 3.11+,
PowerShell 7, Git (including Git Bash), and Docker Desktop running Linux containers
for Linux workflow execution.

```powershell
pwsh -File tools/dev/setup-ci.ps1
pwsh -File tools/dev/ci.ps1
```

Setup downloads checksum-verified act 0.2.98 and Node 26.13.1 under
`target/local-ci/tools`, installs the repository-pinned Rust toolchain with rustfmt/Clippy,
and builds `swoopdl-ci:local`. It does not change the global PATH. The initial
Docker build downloads several GB; later runs reuse Docker and Cargo caches.
Use `setup-ci.ps1 -SkipDocker` if only native checks are needed.
Local Cargo and GitHub Actions read the same `rust-toolchain.toml`, pinned to Rust 1.99.1.
The bootstrap passes that version to the Linux image too, so a newer `stable` release does
not silently change the compiler used for checks.
The native checks use the installed Python; Linux workflows select Python 3.12
where specified by the original workflow. No Python packages are needed by these checks.

## Native pre-push checks

```powershell
pwsh -File tools/dev/ci.ps1                         # all checks
pwsh -File tools/dev/ci.ps1 -Check rust
pwsh -File tools/dev/ci.ps1 -Check extension
pwsh -File tools/dev/ci.ps1 -Check website
pwsh -File tools/dev/ci.ps1 -Check pictures
pwsh -File tools/dev/ci.ps1 -Check translations -Base origin/main
pwsh -File tools/dev/ci.ps1 -DryRun                 # print commands only
```

Native checks mirror CI's format, English catalog, Clippy, workspace tests,
extension tests/packaging/bundle, website tests/build, picture budget, and reviewed
translation guard. Cargo uses `--locked`. The English check uses the extractor's
`--check` mode so it does not rewrite `i18n/en.tsv`. Fetch the comparison branch
before checking translations; a missing base is an error. Checks compare with the
local base ref and do not fetch automatically. A failed check produces a nonzero
exit code. Command output is saved under `target/local-ci/logs`.
Cargo defaults to one compiler job and line-table test debug information to keep builds
within a development machine's memory. Use `-BuildJobs 2` or `4` on larger machines.
The runner restores its temporary environment changes on exit.
Windows Rust tests use a serial harness to avoid native heap crashes between concurrent UI
tests. Linux and macOS retain the default test concurrency.

## Run the actual Linux workflow jobs

```powershell
pwsh -File tools/dev/ci.ps1 -Linux -Job rust
pwsh -File tools/dev/ci.ps1 -Linux -Job extension
pwsh -File tools/dev/ci.ps1 -Linux -Job website
pwsh -File tools/dev/ci.ps1 -Linux -Job pictures
pwsh -File tools/dev/ci.ps1 -Linux -Job website -DryRun
```

These commands invoke act against the existing workflow YAML. Rust selects only
`ubuntu-latest` from the OS matrix. Website selects only `build`, so `deploy`
does not run. Artifacts go under `target/local-ci/artifacts`. No token is needed
for these public-repository checks; network access is needed for Actions and
dependency downloads. Docker builds its own Linux Cargo output; native Windows
output is not copied into the container because `target/` is gitignored.

For Linux/macOS hosts, install act and PowerShell 7, build the same image with
`docker build -t swoopdl-ci:local tools/dev/ci`, and use the same `-Linux` commands.
Native checks also work with Python exposed as `python` and Node 26 on PATH.

## Scope

The native runner mirrors validation commands; update it when CI changes. The act
runner executes the selected YAML job directly. act containers approximate GitHub's
Ubuntu runners; they do not emulate Windows or macOS. Run macOS/Safari tests on a Mac.
See the [act runner documentation](https://nektosact.com/usage/runners.html) for environment differences.
Release packaging, signing/publishing, website deployment, and desktop screenshot
capture are not part of this pre-push command. The screenshot workflow changes host
networking, services, and browser registration and needs its own disposable Windows host.
