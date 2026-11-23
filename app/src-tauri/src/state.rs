//! Application state shared by all commands.

use std::sync::{Arc, Mutex, RwLock};

use bf_core::{SnapshotDiff, Tree};
use bf_scan::CancelToken;

/// Which scan is current. Guarded by one mutex so that "is my scan still current?",
/// storing the tree and emitting the result happen atomically with respect to `start_scan`.
#[derive(Default)]
pub struct ScanSlot {
    /// Id of the most recently started scan (`0` before the first).
    pub id: u64,
    /// Cancel token of that scan, if it has started.
    pub token: Option<CancelToken>,
}

pub struct CompletedScan {
    pub id: u64,
    pub tree: Arc<Tree>,
    /// What was scanned (`C:` or a folder path); used to label snapshots.
    pub target: String,
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
    /// Folder or drive from the `--scan` startup argument, until the front end takes it.
    pub startup_scan: Mutex<Option<String>>,
}
