//! ByteForest core: a compact structure-of-arrays file tree with directory aggregates.
//!
//! Scanners produce a flat stream of [`EntryRecord`]s in any order. [`TreeBuilder`] collects
//! them and [`TreeBuilder::finish`] links them into an immutable [`Tree`].
#![forbid(unsafe_code)]

mod builder;
mod category;
mod diff;
mod flags;
mod record;
mod remove;
mod snapshot;
mod tree;
mod treemap;
mod view;

pub use builder::TreeBuilder;
pub use category::{
    Breakdown, CategoryTotal, ExtTotal, FileCategory, MAX_EXT_LEN, OtherExtensions, category_of,
    extension_of,
};
pub use diff::{DiffEntry, DiffStatus, DiffSummary, Gone, SnapshotDiff, diff};
pub use flags::NodeFlags;
pub use record::EntryRecord;
pub use remove::{RemoveError, Removed};
pub use snapshot::{
    MAX_LABEL_LEN, MAX_NAME_LEN, MAX_NODE_COUNT, MAX_PAYLOAD_LEN, SNAPSHOT_VERSION, Snapshot,
    SnapshotError, read_snapshot, write_snapshot,
};
pub use tree::{DirAgg, NodeId, Tree};
pub use treemap::{Rect, TreemapOptions, header_fits};
pub use view::{FileFilter, NodeView, SearchKind, SearchOutcome, SortKey};
