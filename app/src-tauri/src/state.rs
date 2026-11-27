//! Application state shared by all commands.

use std::sync::{Arc, Mutex, RwLock};

use bf_core::{DuplicateFilter, SnapshotDiff, Tree};
use bf_scan::{CancelToken, DuplicateOutcome};

/// Which scan is current. Guarded by one mutex so that "is my scan still current?",
/// storing the tree and emitting the result happen atomically with respect to `start_scan`.
#[derive(Default)]
pub struct ScanSlot {
    /// Id of the most recently started scan (`0` before the first).
    pub id: u64,
    /// Cancel token of that scan, if it has started.
    pub token: Option<CancelToken>,
    /// A scan was started and has not reported its outcome yet.
    pub running: bool,
}

/// Which duplicate search is current.
#[derive(Default)]
pub struct DupesSlot {
    /// Id of the most recently started duplicate search (`0` before the first).
    pub job_id: u64,
    /// Cancel token of that search, if it has started.
    pub token: Option<CancelToken>,
    /// A duplicate search was started and has not reported its outcome yet.
    pub running: bool,
}

/// Cached duplicate-hashing result for a specific `(scan_id, filter)` pair.
pub struct DupesCache {
    pub scan_id: u64,
    pub filter: DuplicateFilter,
    pub outcome: DuplicateOutcome,
}

/// Where the tree of a [`CompletedScan`] came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Scanned from the disk just now; paths match the disk.
    Live,
    /// Loaded from a snapshot file; paths may no longer match the disk, so nothing in it may
    /// be changed on disk through the app.
    Snapshot { created_unix_ms: i64 },
}

pub struct CompletedScan {
    pub id: u64,
    pub tree: Arc<Tree>,
    /// What was scanned (`C:` or a folder path); used to label snapshots.
    pub target: String,
    pub source: Source,
}

/// A loaded snapshot the current scan is compared against.
pub struct Baseline {
    /// Scan this baseline belongs to; it is dropped when another scan starts.
    pub scan_id: u64,
    /// The snapshot's tree, kept so the diff can be recomputed after a node is trashed.
    pub base: Arc<Tree>,
    /// The comparison, aligned to the node ids of the scan's tree.
    pub diff: Arc<SnapshotDiff>,
}

/// The last finished scan and the current scan slot.
#[derive(Default)]
pub struct AppState {
    /// Tree of the last finished scan. Readers clone the `Arc` and drop the lock at once.
    /// Lock order: `scan` first, then `tree`.
    pub tree: RwLock<Option<CompletedScan>>,
    /// Snapshot comparison for the scan in `tree`, if the user started one.
    /// Lock order: `scan`, then `tree`, then `baseline`; never hold `baseline` while
    /// acquiring either of the others.
    pub baseline: RwLock<Option<Baseline>>,
    /// Current scan id and cancel token.
    pub scan: Mutex<ScanSlot>,
    /// Current duplicate search job id and cancel token.
    /// Lock order: `scan`, then `tree`, then `dupes_job`, then `dupes_cache`; never hold
    /// `dupes_job` or `dupes_cache` while acquiring `scan` or `tree`.
    pub dupes_job: Mutex<DupesSlot>,
    /// Last confirmed duplicate search outcome for the current scan.
    /// Lock order: `scan`, then `tree`, then `dupes_job`, then `dupes_cache`; never hold
    /// `dupes_cache` while acquiring any of the others.
    pub dupes_cache: RwLock<Option<DupesCache>>,
    /// Folder or drive from the `--scan` startup argument, until the front end takes it.
    pub startup_scan: Mutex<Option<String>>,
}
