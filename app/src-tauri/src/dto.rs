//! Payloads of the events sent to the front end.

use bf_core::NodeView;
use bf_scan::ScanStats;
use serde::Serialize;

/// Payload of `scan://done`.
#[derive(Serialize, Clone, Debug)]
pub struct ScanDone {
    /// Summary of the finished scan.
    pub stats: ScanStats,
    /// The root node of the new tree.
    pub root: NodeView,
}

/// Payload of `scan://error` (also used for a cancelled scan).
#[derive(Serialize, Clone, Debug)]
pub struct ScanErrorEvent {
    /// Human-readable reason.
    pub message: String,
}
