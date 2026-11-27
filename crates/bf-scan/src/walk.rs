use std::fs::ReadDir;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use bf_core::{EntryRecord, NodeFlags, TreeBuilder};

use crate::progress::{Phase, ProgressReporter};
#[cfg(windows)]
use crate::volumes::cluster_size;
use crate::{CancelToken, ProgressFn, ScanError, ScanMethod, ScanRoot, ScanStats, Scanner};

/// Records collected per worker before they are moved to the shared list.
const BATCH: usize = 4096;
/// Key of the root node; every other key comes from `Shared::next_key`.
const ROOT_KEY: u64 = 0;

// Win32 file attribute bits.
#[cfg(windows)]
const ATTR_HIDDEN: u32 = 0x2;
#[cfg(windows)]
const ATTR_SYSTEM: u32 = 0x4;
#[cfg(windows)]
const ATTR_DIRECTORY: u32 = 0x10;
#[cfg(windows)]
const ATTR_SPARSE: u32 = 0x200;
#[cfg(windows)]
const ATTR_REPARSE_POINT: u32 = 0x400;
#[cfg(windows)]
const ATTR_COMPRESSED: u32 = 0x800;
#[cfg(windows)]
const ATTR_OFFLINE: u32 = 0x1000;
#[cfg(windows)]
const ATTR_RECALL_ON_OPEN: u32 = 0x4_0000;
#[cfg(windows)]
const ATTR_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;

/// Scanner that walks directories with `std::fs`, one rayon task per directory.
///
/// Works on any file system, without elevation, and on plain folders. Directory reparse
/// points (junctions, symlinks) are recorded with size 0 and never entered; file reparse
/// points (WOF-compressed files, cloud files) keep their logical size. Hard links are not
/// de-duplicated.
pub struct WalkScanner;

/// State shared by all worker tasks.
struct Shared<'a> {
    #[cfg(windows)]
    cluster: u64,
    #[cfg(target_os = "macos")]
    device: u64,
    next_key: AtomicU64,
    errors: AtomicU64,
    records: Mutex<Vec<EntryRecord>>,
    progress: ProgressReporter<'a>,
    cancel: &'a CancelToken,
}

impl Scanner for WalkScanner {
    fn scan(
        &self,
        root: &ScanRoot,
        progress: ProgressFn<'_>,
        cancel: &CancelToken,
    ) -> Result<(bf_core::Tree, ScanStats), ScanError> {
        let start = Instant::now();
        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        let (root_path, root_name) = resolve_root(root)?;

        // Reading the root itself must succeed; failures below it are only counted.
        let root_entries = std::fs::read_dir(&root_path).map_err(root_error)?;

        let shared = Shared {
            #[cfg(windows)]
            cluster: cluster_size(&root_path),
            #[cfg(target_os = "macos")]
            device: std::fs::metadata(&root_path).map_err(root_error)?.dev(),
            next_key: AtomicU64::new(ROOT_KEY + 1),
            errors: AtomicU64::new(0),
            records: Mutex::new(Vec::new()),
            progress: ProgressReporter::new(progress),
            cancel,
        };
        rayon::scope(|scope| process_dir(scope, &shared, &root_path, root_entries, ROOT_KEY));

        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }

        // The counters now hold the final totals, so this one call is both the "Building"
        // notification and the closing progress report.
        shared.progress.finish(Phase::Building);
        let errors = shared.errors.load(Ordering::Relaxed);
        let records = shared
            .records
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        let mut builder = TreeBuilder::new(ROOT_KEY, &root_name);
        for rec in records {
            builder.push(rec);
        }
        let tree = builder.finish();

        let total = tree.total(tree.root());
        let stats = ScanStats {
            files: total.files,
            dirs: total.dirs,
            bytes: total.size,
            elapsed_ms: start.elapsed().as_millis() as u64,
            method: ScanMethod::Walk,
            fallback_reason: None,
            skipped_records: 0,
            errors,
        };
        Ok((tree, stats))
    }
}

/// Returns the directory to read and the name of the root node.
fn resolve_root(root: &ScanRoot) -> Result<(PathBuf, String), ScanError> {
    match root {
        #[cfg(windows)]
        ScanRoot::Volume(letter) => {
            if !letter.is_ascii_alphabetic() {
                return Err(ScanError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid drive letter {letter:?}"),
                )));
            }
            let letter = letter.to_ascii_uppercase();
            Ok((PathBuf::from(format!("{letter}:\\")), format!("{letter}:")))
        }
        ScanRoot::Folder(path) => Ok((path.clone(), path.display().to_string())),
    }
}

fn root_error(err: io::Error) -> ScanError {
    if err.kind() == io::ErrorKind::PermissionDenied {
        ScanError::AccessDenied
    } else {
        ScanError::Io(err)
    }
}

/// Opens `path` and processes it; a failure is counted and the directory is skipped.
fn walk_dir<'s>(scope: &rayon::Scope<'s>, shared: &'s Shared<'s>, path: PathBuf, key: u64) {
    if shared.cancel.is_cancelled() {
        return;
    }
    match std::fs::read_dir(&path) {
        Ok(entries) => process_dir(scope, shared, &path, entries, key),
        Err(_) => {
            shared.errors.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Turns the entries of one directory into records and spawns a task per subdirectory.
fn process_dir<'s>(
    scope: &rayon::Scope<'s>,
    shared: &'s Shared<'s>,
    dir: &Path,
    entries: ReadDir,
    dir_key: u64,
) {
    let mut batch: Vec<EntryRecord> = Vec::new();
    // Counters for this task, reported and reset at every flush.
    let mut counts = Counts::default();

    for entry in entries {
        if shared.cancel.is_cancelled() {
            return;
        }
        let Ok(entry) = entry else {
            shared.errors.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        // On Windows this does not follow reparse points.
        let Ok(meta) = entry.metadata() else {
            shared.errors.fetch_add(1, Ordering::Relaxed);
            continue;
        };

        #[cfg(target_os = "macos")]
        if meta.dev() != shared.device {
            continue;
        }
        #[cfg(windows)]
        let (flags, size, alloc, mtime) = {
            let attrs = meta.file_attributes();
            let (size, alloc) = entry_sizes(attrs, meta.len(), shared.cluster);
            (
                flags_from_attributes(attrs),
                size,
                alloc,
                meta.last_write_time() as i64,
            )
        };
        #[cfg(unix)]
        let (flags, size, alloc, mtime) = unix_entry(&meta);

        let is_dir = flags.contains(NodeFlags::DIR);
        let key = shared.next_key.fetch_add(1, Ordering::Relaxed);
        batch.push(EntryRecord {
            key,
            parent_key: dir_key,
            name: entry.file_name().to_string_lossy().into_owned(),
            size,
            alloc,
            mtime,
            flags,
        });

        if is_dir {
            counts.dirs += 1;
            if !flags.contains(NodeFlags::REPARSE) {
                let child = dir.join(entry.file_name());
                scope.spawn(move |s| walk_dir(s, shared, child, key));
            }
        } else {
            counts.files += 1;
            counts.bytes += size;
        }

        if batch.len() >= BATCH {
            flush(shared, &mut batch, &mut counts);
        }
    }
    flush(shared, &mut batch, &mut counts);
}

/// Per-task counters that are added to the shared progress at every flush.
#[derive(Default)]
struct Counts {
    files: u64,
    dirs: u64,
    bytes: u64,
}

/// Moves a worker's records to the shared list and reports (then resets) its counters.
fn flush(shared: &Shared<'_>, batch: &mut Vec<EntryRecord>, counts: &mut Counts) {
    if !batch.is_empty() {
        shared
            .records
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .append(batch);
    }
    let c = std::mem::take(counts);
    shared
        .progress
        .tick(c.files, c.dirs, c.bytes, Phase::Walking);
}

#[cfg(windows)]
fn flags_from_attributes(attrs: u32) -> NodeFlags {
    let mut flags = NodeFlags::empty();
    for (bit, flag) in [
        (ATTR_DIRECTORY, NodeFlags::DIR),
        (ATTR_HIDDEN, NodeFlags::HIDDEN),
        (ATTR_SYSTEM, NodeFlags::SYSTEM),
        (ATTR_REPARSE_POINT, NodeFlags::REPARSE),
        (ATTR_COMPRESSED, NodeFlags::COMPRESSED),
        (ATTR_SPARSE, NodeFlags::SPARSE),
    ] {
        if attrs & bit != 0 {
            flags |= flag;
        }
    }
    flags
}

/// Logical and allocated size of an entry with attributes `attrs` and length `len`.
///
/// Directories (junctions included) count 0. Files, including file reparse points such as
/// WOF-compressed system files, keep their logical size; their alloc is `len` rounded up to
/// clusters, except for cloud placeholders whose data is not on the disk (alloc 0).
#[cfg(windows)]
fn entry_sizes(attrs: u32, len: u64, cluster: u64) -> (u64, u64) {
    let not_on_disk = ATTR_OFFLINE | ATTR_RECALL_ON_OPEN | ATTR_RECALL_ON_DATA_ACCESS;
    if attrs & ATTR_DIRECTORY != 0 {
        (0, 0)
    } else if attrs & ATTR_REPARSE_POINT != 0 && attrs & not_on_disk != 0 {
        (len, 0)
    } else {
        (len, round_up(len, cluster))
    }
}

/// Rounds `size` up to a multiple of `cluster` (saturating; `cluster` is never 0).
#[cfg(windows)]
fn round_up(size: u64, cluster: u64) -> u64 {
    size.div_ceil(cluster).saturating_mul(cluster)
}

#[cfg(unix)]
fn unix_entry(meta: &std::fs::Metadata) -> (NodeFlags, u64, u64, i64) {
    let flags = if meta.is_dir() {
        NodeFlags::DIR
    } else if meta.is_symlink() {
        NodeFlags::REPARSE
    } else {
        NodeFlags::empty()
    };
    let (size, alloc) = if meta.is_file() {
        (meta.len(), meta.blocks().saturating_mul(512))
    } else {
        (0, 0)
    };
    // Keep the existing FILETIME representation for the UI and CLI.
    let ticks = (i128::from(meta.mtime()) + 11_644_473_600) * 10_000_000
        + i128::from(meta.mtime_nsec()) / 100;
    let mtime = i64::try_from(ticks)
        .ok()
        .filter(|value| *value > 0)
        .unwrap_or(0);
    (flags, size, alloc, mtime)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    const ATTR_ARCHIVE: u32 = 0x20;

    #[test]
    fn directories_have_no_size() {
        assert_eq!(entry_sizes(ATTR_DIRECTORY, 4096, 4096), (0, 0));
        // A directory reparse point (junction) as well.
        assert_eq!(
            entry_sizes(ATTR_DIRECTORY | ATTR_REPARSE_POINT, 4096, 4096),
            (0, 0)
        );
    }

    #[test]
    fn plain_files_round_alloc_up_to_clusters() {
        assert_eq!(entry_sizes(ATTR_ARCHIVE, 10, 4096), (10, 4096));
        assert_eq!(entry_sizes(0, 0, 4096), (0, 0));
    }

    #[test]
    fn file_reparse_points_keep_their_logical_size() {
        // WOF-compressed system files and hydrated cloud files.
        assert_eq!(entry_sizes(ATTR_REPARSE_POINT, 5000, 4096), (5000, 8192));
    }

    #[test]
    fn cloud_placeholders_take_no_space() {
        for bit in [
            ATTR_OFFLINE,
            ATTR_RECALL_ON_OPEN,
            ATTR_RECALL_ON_DATA_ACCESS,
        ] {
            assert_eq!(
                entry_sizes(ATTR_REPARSE_POINT | bit, 5000, 4096),
                (5000, 0),
                "bit {bit:#x}"
            );
        }
        // The placeholder rule only applies to reparse points.
        assert_eq!(entry_sizes(ATTR_OFFLINE, 5000, 4096), (5000, 8192));
    }
}
