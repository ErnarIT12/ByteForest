//! Application state shared by all commands.

use std::sync::{Arc, Mutex, RwLock};

use bf_core::Tree;
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
}

/// The last finished scan and the current scan slot.
#[derive(Default)]
pub struct AppState {
    /// Tree of the last finished scan. Readers clone the `Arc` and drop the lock at once.
    /// Lock order: `scan` first, then `tree`.
    pub tree: RwLock<Option<CompletedScan>>,
    /// Current scan id and cancel token.
    pub scan: Mutex<ScanSlot>,
    /// Folder or drive from the `--scan` startup argument, until the front end takes it.
    pub startup_scan: Mutex<Option<String>>,
}
