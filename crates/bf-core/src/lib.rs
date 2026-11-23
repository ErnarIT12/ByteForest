//! ByteForest core: a compact structure-of-arrays file tree with directory aggregates.
//!
//! Scanners produce a flat stream of [`EntryRecord`]s in any order. [`TreeBuilder`] collects
//! them and [`TreeBuilder::finish`] links them into an immutable [`Tree`].
#![forbid(unsafe_code)]

mod builder;
mod category;
mod diff;
mod exclude;
mod flags;
mod record;
mod remove;
mod report;
mod snapshot;
mod sunburst;
mod tree;
mod treemap;
mod view;

pub use builder::TreeBuilder;
pub use category::{
    Breakdown, CategoryTotal, ExtTotal, FileCategory, MAX_EXT_LEN, OtherExtensions, category_of,
    extension_of,
};
pub use diff::{DiffEntry, DiffStatus, DiffSummary, Gone, SnapshotDiff, diff};
pub use exclude::{
    Excluded, ExclusionSet, MAX_EXCLUSION_PATTERN_LEN, MAX_EXCLUSION_PATTERNS, NamePattern,
    normalize_patterns,
};
pub use flags::NodeFlags;
pub use record::EntryRecord;
pub use remove::{RemoveError, Removed};
pub use report::{MAX_REPORT_DEPTH, MAX_REPORT_ROWS, Report, ReportOptions, ReportRow, write_csv};
pub use snapshot::{
    MAX_COMPRESSION_RATIO, MAX_LABEL_LEN, MAX_NAME_LEN, MAX_NODE_COUNT, MAX_PAYLOAD_LEN,
    SNAPSHOT_VERSION, Snapshot, SnapshotError, SnapshotLimits, read_snapshot,
    read_snapshot_with_limits, write_snapshot,
};
pub use sunburst::{MAX_SUNBURST_DEPTH, OTHER_ARC_ID, SunburstArc, SunburstOptions};
pub use tree::{DirAgg, NodeId, Tree};
pub use treemap::{Rect, TreemapOptions, header_fits};
pub use view::{FileFilter, NodeView, SearchKind, SearchOutcome, SortKey};
