//! Payloads of the events sent to the front end.
//!
//! Every event carries the `scan_id` returned by `start_scan`; the UI must ignore events
//! whose id is not the latest one it started.

use bf_core::NodeView;
use bf_scan::{ScanProgress, ScanStats};
use serde::Serialize;

/// Payload of `scan://progress`: the counters plus the scan id.
#[derive(Serialize, Clone, Debug)]
pub struct ScanProgressEvent {
    /// Scan this update belongs to.
    pub scan_id: u64,
    /// The counters, flattened into the same JSON object.
    #[serde(flatten)]
    pub progress: ScanProgress,
}

/// Payload of `scan://done`.
#[derive(Serialize, Clone, Debug)]
pub struct ScanDone {
    /// Scan that finished.
    pub scan_id: u64,
    /// Summary of the finished scan.
    pub stats: ScanStats,
    /// The root node of the new tree.
    pub root: NodeView,
}

/// Payload of `scan://error` (also used for a cancelled scan).
#[derive(Serialize, Clone, Debug)]
pub struct ScanErrorEvent {
    /// Scan that failed.
    pub scan_id: u64,
    /// Human-readable reason.
    pub message: String,
}
