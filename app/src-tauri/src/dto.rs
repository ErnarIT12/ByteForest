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

/// One treemap rectangle as sent to the UI: the layout rectangle plus what the canvas needs
/// to label it, colour it by ancestor and show a tooltip without extra round trips.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct TreemapRect {
    /// Node index.
    pub id: u32,
    /// Node index of the parent (`None` for the tree root).
    pub parent: Option<u32>,
    /// File or folder name.
    pub name: String,
    /// Logical size of the subtree in bytes.
    pub size: u64,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Distance from the requested node (which is depth 0).
    pub depth: u8,
    pub is_dir: bool,
}
