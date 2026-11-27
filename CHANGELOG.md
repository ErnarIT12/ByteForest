# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- SwiftUI-inspired visual polish across Graphite and Silver: introduce the ByteForest emerald/forest accent (WCAG AA), replace heavy 1 px control outlines with layered surface elevation, unify 32 px toolbar controls and 8 px / 12 px radii, and redesign **Largest files** with a single inline search bar, visible space-hog/cache preset cards showing reclaimable space, Treemap-colored category dots without zero-count clutter, parent-folder-only locations and hover quick actions.

- Duplicate Finder: find exact duplicate files from the **Duplicates** view in the sidebar or "Find duplicates here" on any folder. Estimates candidates instantly in memory, verifies matches in three steps (size → partial hash → full BLAKE3 on a bounded I/O thread pool) while skipping hardlinks, reparse points and cloud placeholders, and supports an optional hidden/system file toggle, category/size/name filters, auto-select presets (Keep oldest, Keep newest, Keep shortest path) with an all-copies-queued warning, cached results that prune automatically after cleanup, and direct Cleanup queue integration. The CLI gets `byteforest dupes <target>` with `--min-size`, `--top`, `--exclude`, `--include-hidden` and `--json`.

- Show available space on the scan start screen, with a compact refresh control in the results toolbar. The capacity ring and disk maps appear with short transitions that follow system motion preferences.

- Explore largest files in a full-width table with type counts, size filters, file icons and a reviewed cleanup selection. Windows can display executable icons from the shell.
- Switch among Disk map, Largest files, Scan history and Cleanup in the sidebar. Resize the tree beside the map, compare saved scans and open Settings over the current view.

- Center the desktop window on launch and constrain its initial size to the work area. Choose a folder directly from the welcome screen, with reduced-motion-aware transitions into scanning and results.

## [0.4.0] - 2026-11-24

This release also contains everything planned for 0.3.0, which was never released separately.

### Added

- Discard Pile: queue files and folders from the folder tree, treemap, sunburst or inspector (right-click, the Delete key, or the inspector button; Shift+Delete still moves one item right away), review the item count and total size in the sidebar, and move everything to the Recycle Bin/Trash in one confirmed step. Items that cannot be moved stay queued and are listed with the reason, and the tree, treemap and totals update once without losing expanded folders. Queued items are marked in the tree and on the maps.
- Sunburst disk chart (Treemap | Sunburst switch in the top bar): rings of the focused folder with the same zoom, selection, tooltip, breadcrumbs, keyboard (arrows move between siblings, parent and largest child), context menu, Discard Pile markers and Folders / File types colors as the treemap; the centre shows the folder and goes up when clicked. Tiny items of a folder are merged into one "other" arc, and the chart shows up to six rings and 20,000 arcs.
- Snapshots and compare mode: save a scan as a compact snapshot file (Snapshots, Save snapshot…) and compare a later scan with it (Compare with snapshot…). The folder tree shows what grew, shrank or is new, a Changes view lists the biggest increases, and the file inspector reports what is gone. The CLI gets `byteforest scan <target> --save-snapshot <FILE>` and `--diff <FILE>`, with JSON output for `--diff`.
- Diff highlights on the map while comparing with a snapshot, in both the treemap and the sunburst: grown items get a solid outline, new items a dotted outline and a "New" label, shrunk items a dashed outline, with the size change (such as +4.8 GB) shown where it fits and a Grown / New / Shrunk key under the toolbar. The tooltip also reports the change.
- Automatic snapshots and instant reopen: every finished scan is saved in the background to the app's local data folder (`snapshots/auto`; the last 10 are kept by default, 0 to 50 in Settings, Off to disable) without delaying the result. Recent Scans in the sidebar lists the saved snapshots per location with their age and size; click one to open it instantly and read-only (a banner and a Snapshot badge say so, and moving to the Recycle Bin/Trash or the Discard Pile is disabled), compare it with the current scan, or delete it. Snapshots contain file and folder names and sizes and never leave your computer.
- Scan exclusions in Settings (Excluded from scans): names and wildcards such as `node_modules` or `*.tmp` match in any folder, and an absolute path such as `C:\Windows\WinSxS` matches only that folder. Excluded items are left out of the tree, the treemap and all totals from the next scan on, and the status bar shows how much was excluded. The CLI gets a repeatable `byteforest scan --exclude <PATTERN>`.
- CSV or JSON report of any folder (Export report… in the file inspector): Excel-friendly UTF-8 CSV with formula-injection protection, or JSON. The CLI gets `byteforest scan --export <FILE>` with `--export-depth` and `--export-rows`.
- One-click search presets (node_modules, target, .venv, .git, disk images, files over 1 GB) that find outermost build and cache folders or large files and show the total size reclaimable.
- File-type breakdown (categories and top extensions) for the selected folder, with click-through to a search limited to that folder, and a Folders / File types color mode for the treemap with a legend. The size of folders themselves ("Folder metadata") is shown so the total matches the folder size.
- Right-click context menu (also Shift+F10 or the Menu key) on the folder tree and treemap, with Open folder, Show in Explorer/Finder, Copy Path and Move to Recycle Bin/Trash after confirmation. The tree, treemap and totals update instantly without a rescan.
- Keyboard navigation of the treemap: arrow keys move the selection, Enter opens the selected folder, and Escape or Backspace goes up a level.
- Left sidebar with drives (with available space and disk usage shown before scanning), Desktop, Downloads and Documents, recent scans and a Discard Pile dock; it collapses to an icon rail.
- Graphite, Silver and System appearances, and the silver ByteForest mark across the desktop app.
- Hover feedback on primary buttons in both Graphite and Silver, respecting the reduced-motion setting.

### Changed

- Graphite (dark) is the default appearance on fresh installs. Silver (light) and System are available in Settings, and the title bar and logo follow the selected theme.
- Redesign the workspace: Treemap | Sunburst switch in the top bar, Lucide icons throughout, soft elevation instead of panel borders in Graphite and Silver, a title bar integrated with the workspace, an expandable disk map, and the map beside a right-hand file browser and file inspector that shows the selected item's share of the scan.
- Choose a scan location from a list with available space.
- Open (zoom into) a treemap folder with a double click. A single click selects it.
- Keep expanded folders and the selection when starting or stopping a snapshot comparison.
- Hide parent folders in the Changes list when a single file or folder inside accounts for their whole increase.
- Remove the "⚡" icon from the Fast scan (admin) label.
- Hide the "Always run as administrator" setting on macOS.
- Update all READMEs as a v0.4.0 showcase with screenshots, and credit Radix as a UX inspiration.

### Fixed

- Saving one setting no longer wipes the others: the settings popover now always sends the complete settings.

### Security

- Refuse oversized or highly compressed snapshot files before decoding them, so a crafted file cannot exhaust memory.

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

[Unreleased]: https://github.com/ErnarIT12/byteforest/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/ErnarIT12/byteforest/compare/v0.2.0...v0.4.0
[0.2.0]: https://github.com/ErnarIT12/byteforest/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/ErnarIT12/byteforest/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ErnarIT12/byteforest/releases/tag/v0.1.0
