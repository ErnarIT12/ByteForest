# ByteForest

**A fast, open-source disk space analyzer for Windows: see what fills your drive in seconds.**

[![CI](https://github.com/ErnarIT12/byteforest/actions/workflows/ci.yml/badge.svg)](https://github.com/ErnarIT12/byteforest/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

English | [Русский](README.ru.md)

<!-- ![ByteForest](docs/screenshot.png) -->

## Why ByteForest

- **Fast.** On NTFS volumes ByteForest reads the Master File Table directly instead of asking Windows about every folder one by one. A drive with over a million entries is scanned in a couple of seconds.
- **Works when raw reads are blocked.** Some anti-cheat drivers block raw volume access. ByteForest detects this and falls back to reading MFT records one by one through the file system, which is still fast. Without administrator rights it uses a regular directory walker.
- **Compact UI.** A folder tree with sizes and percentages next to an interactive treemap. Selection is synchronized between both.
- **Free and open source.** No telemetry, no ads. Dual-licensed under MIT or Apache-2.0.

## Measured performance

Measured on a Windows 11 machine with an NVMe drive:

| Volume | Mode | Entries (files + folders) | Scan time |
|---|---|---|---|
| `C:` | MFT, elevated (record-by-record, anti-cheat driver blocks raw reads) | 1 252 993 | ~2.2 s |
| `E:` | MFT, elevated (raw MFT blocks) | 365 585 | ~0.6 s |
| `C:` | Directory walker, not elevated | about 1.2 million | ~58 s |

These are single-machine numbers, not a formal benchmark. You can measure your own with `byteforest bench` (see [CLI usage](#cli-usage)).

## Features (v0.1)

- Fast NTFS scanner that reads the MFT, with automatic fallback to a directory walker when there are no admin rights or the volume is not NTFS
- Folder tree with size, share of parent, file count and modification date
- Treemap (Canvas) with zoom into a folder and breadcrumbs
- Tooltips and selection synchronized between tree, treemap and status bar
- Light and dark theme that follows the system
- Command-line tool: list drives, scan and print the largest folders, benchmark
- No telemetry; logs stay on your machine

## Roadmap

| Version | Planned |
|---|---|
| v0.2 | Overview screen, details panel, Mica and themes, move to Recycle Bin, restart as administrator, Russian and English UI, settings |
| v0.3 | Search and filter chips, for example `ext:mp4 size:>1GB` |
| v0.4 | Snapshots, diff between scans, "Changes" screen |
| v0.5 | Cleanup rules and cleanup potential |
| v0.6 | Duplicate finder (hard links excluded) |
| v1.0 | Benchmarks, fuzzing, auto-update, documentation, winget package, code signing |

### Known limitations

- Memory use is currently about 400 bytes per entry. The goal for v1.0 is 120 bytes or less.
- Windows 10/11 x64 only. ReFS and other file systems use the slower directory walker.
- Hard links are not de-duplicated yet, so the walker may report more space than is really used.

## Install

1. Open the [Releases](https://github.com/ErnarIT12/byteforest/releases) page and download the installer for the latest version.
2. Run it.

Builds are **not code-signed yet**, so Windows SmartScreen will warn you about an unknown publisher. Choose "More info", then "Run anyway" if you trust the download. Checksums are published with each release.

Run ByteForest **as administrator** to enable fast MFT mode. Without admin rights it still works, using the slower directory walker.

## CLI usage

```text
byteforest drives                  # list available drives
byteforest scan C: --top 20        # scan a drive and print the 20 largest items
byteforest scan C: --top 20 --json # same, as JSON
byteforest bench C: --runs 3       # time the scan three times
```

Run the CLI from an elevated terminal to use the MFT scanner.

## Build from source

Requirements: Rust stable (MSVC toolchain), Node 24 or newer, and the Visual Studio Build Tools with the Windows SDK.

```powershell
git clone https://github.com/ErnarIT12/byteforest.git
cd byteforest

# frontend dependencies and dev build of the app
npm --prefix app ci
npm --prefix app run tauri dev

# tests (build the frontend first, the app crate embeds it)
npm --prefix app run build
cargo test --workspace
```

To also run the tests that compare the MFT scanner with the directory walker on a real volume, use an elevated shell and set `BF_ADMIN_TESTS=1`. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Architecture

```text
crates/
  bf-ntfs   pure NTFS parsing over byte slices (no Windows APIs)
  bf-core   tree model, aggregates, treemap layout (no Windows APIs)
  bf-scan   scanners: MftScanner and WalkScanner
  bf-cli    the byteforest command-line tool
app/
  src-tauri Tauri commands and events, a thin layer over the crates
  src       React + TypeScript user interface
```

The scanned tree lives in Rust and never leaves it. The UI asks for slices: the children of one folder (at most 1000 nodes) or the rectangles for one treemap view (at most 20 000). This keeps the interface smooth on drives with millions of files, because the web view only ever holds what is on screen.

## Contributing

Issues and pull requests are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md) first. To report a security problem see [SECURITY.md](SECURITY.md).

## Acknowledgements

Inspired by disk analyzers such as WinDirStat and WizTree.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
