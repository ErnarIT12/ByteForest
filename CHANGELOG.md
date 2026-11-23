# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-11-23

- Search the entire scan by file name, extension, and minimum size in Largest Files. Select a result to jump to it in the tree and on the map.
- View the path, size, allocated size estimate, and modification date. Copy the path or reveal the selection in Explorer or Finder.
- macOS beta for Apple Silicon and Intel. Scan the home folder, local volumes, and folders chosen in the system picker. Downloads include a universal DMG.

## [0.1.1] - 2026-11-16

### Added

- "⚡ Fast scan (admin)" button: restarts ByteForest with administrator rights and resumes the scan of the selected drive automatically.
- "Always run as administrator" setting, stored in `%APPDATA%\ByteForest\settings.json`.
- Elevation hint bar after a slow standard-mode scan, with a one-click restart as administrator.
- `--scan <drive>` startup argument of the app: scans the given local drive (`X:`, `X:\` or `X:\path`) right after start. UNC and relative paths are ignored.
- Logo as the application icon and in the window.
- Brand theme (light and dark) in the logo palette, with automated WCAG AA contrast tests.
- Multilingual README (English, Russian, Kazakh, Chinese, Spanish, German, Japanese) with screenshots and a script that creates the demo data (`scripts/make-demo-data.ps1`).

### Changed

- Security and conduct reports go through GitHub (private vulnerability reporting) instead of email.
- The NSIS installer installs per machine (into Program Files), so "Always run as administrator" cannot be abused by replacing a user-writable executable.
- Fallback message when raw disk reads are blocked: "Raw disk reads are blocked (likely an anti-cheat driver); reading MFT records one by one".
- Tests use the `BF_TEST_TMP` variable instead of machine-specific temporary paths.

### Fixed

- Paths ending with a backslash are quoted correctly when restarting as administrator.
- Elevation bar layout, and scans of a folder no longer offer elevation.
- Dates use the English format in the interface.
- Scan durations under 0.1 s are shown as `<0.1 s`.

### Security

- The startup `--scan` argument accepts only local drive paths, so a crafted shortcut cannot start a scan of a network share.
- Settings are not read or written when `APPDATA` is unset (no fallback to the current folder).
- The elevation request uses `SEE_MASK_NOASYNC` and `SEE_MASK_FLAG_NO_UI`.

## [0.1.0] - 2026-11-11

### Added

- `bf-ntfs`: pure NTFS parser for the `NTFS_VOLUME_DATA_BUFFER` returned by `FSCTL_GET_NTFS_VOLUME_DATA`, FILE records with update-sequence fixups, `$STANDARD_INFORMATION`, `$FILE_NAME`, `$DATA` and data runs. Corrupted records are skipped instead of failing the scan. A file's size is the logical size of its unnamed `$DATA` stream; named streams (alternate data streams, `WofCompressedData`) count only toward the allocated size.
- `bf-scan`: `MftScanner` that reads the MFT in raw blocks and merges extension records into their base record through the base-record reference. When raw volume reads are blocked (for example by an anti-cheat driver) or fail during the scan, it reads the MFT record by record with `FSCTL_GET_NTFS_FILE_RECORD` instead and reports why. `WalkScanner` is a directory walker used without administrator rights, on non-NTFS volumes, or when the MFT scan fails.
- `bf-core`: tree model with size and file count aggregates, and treemap layout.
- `byteforest` command-line tool: `drives`, `scan <drive> --top N [--json]` and `bench <drive> --runs N`.
- Desktop app (Tauri 2, React): folder tree with size, share, file count and date; Canvas treemap with zoom and breadcrumbs; synchronized selection; status bar; light and dark theme following the system.
- Repository documentation, dual MIT OR Apache-2.0 licensing, issue and pull request templates, Dependabot configuration, `cargo-deny` policy and GitHub Actions CI.

### Known issues

- Memory use is about 400 bytes per entry; the goal for v1.0 is 120 bytes or less.
- Hard links are not de-duplicated by the directory walker.
- Builds are not code-signed, so Windows SmartScreen shows a warning.

[Unreleased]: https://github.com/ErnarIT12/byteforest/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/ErnarIT12/byteforest/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/ErnarIT12/byteforest/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ErnarIT12/byteforest/releases/tag/v0.1.0
