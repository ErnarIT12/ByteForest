# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/ErnarIT12/byteforest/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ErnarIT12/byteforest/releases/tag/v0.1.0
