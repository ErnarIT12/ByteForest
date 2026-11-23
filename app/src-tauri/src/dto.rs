//! Payloads of the events sent to the front end.
//!
//! Every event carries the `scan_id` returned by `start_scan`; the UI must ignore events
//! whose id is not the latest one it started.

use bf_core::{DiffStatus, Gone, NodeId, NodeView, SnapshotDiff, Tree};
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
    /// File category; `None` for directories.
    pub category: Option<bf_core::FileCategory>,
}

/// How a node differs from the snapshot being compared against.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiffKind {
    Added,
    Grown,
    Shrunk,
    Unchanged,
}

impl From<DiffStatus> for DiffKind {
    fn from(status: DiffStatus) -> Self {
        match status {
            DiffStatus::Added => DiffKind::Added,
            DiffStatus::Grown => DiffKind::Grown,
            DiffStatus::Shrunk => DiffKind::Shrunk,
            DiffStatus::Unchanged => DiffKind::Unchanged,
        }
    }
}

/// Bytes and entry count of removed or added entries.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoneDto {
    pub bytes: u64,
    pub count: u64,
}

impl From<Gone> for GoneDto {
    fn from(g: Gone) -> Self {
        GoneDto {
            bytes: g.bytes,
            count: g.count,
        }
    }
}

/// Per-node comparison result sent with a node.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct DiffDto {
    pub status: DiffKind,
    /// Size in the snapshot; `None` for added nodes.
    pub base: Option<u64>,
    pub delta: i64,
    /// Things inside this folder that are in the snapshot but gone now.
    pub removed: Option<GoneDto>,
}

/// Builds the diff payload for a node; `None` when nothing differs.
pub fn diff_dto(diff: &SnapshotDiff, tree: &Tree, id: NodeId) -> Option<DiffDto> {
    let entry = diff.entry(tree, id);
    let gone = diff.removed_under(id);
    let removed = (gone.count > 0).then(|| GoneDto::from(gone));
    if entry.status == DiffStatus::Unchanged && removed.is_none() {
        return None;
    }
    Some(DiffDto {
        status: entry.status.into(),
        base: entry.base,
        delta: entry.delta,
        removed,
    })
}

/// A node plus, while comparing with a snapshot, how it changed.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct NodeViewDto {
    #[serde(flatten)]
    pub view: NodeView,
    pub diff: Option<DiffDto>,
}

impl NodeViewDto {
    /// Attaches the comparison result when a diff is active.
    pub fn new(view: NodeView, diff: Option<&SnapshotDiff>, tree: &Tree) -> Self {
        let dto = diff.and_then(|d| diff_dto(d, tree, NodeId(view.id)));
        NodeViewDto { view, diff: dto }
    }
}

/// Summary returned when a comparison starts.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct BaselineInfo {
    pub label: String,
    pub created_unix_ms: i64,
    pub total_delta: i64,
    pub added: GoneDto,
    pub removed: GoneDto,
}

/// One row of the "Changes" list.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct ChangeDto {
    pub id: u32,
    pub name: String,
    pub path: String,
    pub status: DiffKind,
    pub base: Option<u64>,
    pub current: u64,
    pub delta: i64,
}
