//! Application state shared by all commands.

use std::sync::{Arc, Mutex, RwLock};

use bf_core::Tree;
use bf_scan::CancelToken;

/// The last finished scan and the token of the scan that is running now.
#[derive(Default)]
pub struct AppState {
    /// Tree of the last finished scan. Readers clone the `Arc` and drop the lock at once.
    pub tree: RwLock<Option<Arc<Tree>>>,
    /// Cancel token of the running scan, if any.
    pub cancel: Mutex<Option<CancelToken>>,
}
