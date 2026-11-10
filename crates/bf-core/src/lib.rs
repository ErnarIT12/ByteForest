//! ByteForest core: a compact structure-of-arrays file tree with directory aggregates.
//!
//! Scanners produce a flat stream of [`EntryRecord`]s in any order. [`TreeBuilder`] collects
//! them and [`TreeBuilder::finish`] links them into an immutable [`Tree`].

mod builder;
mod flags;
mod record;
mod tree;
mod view;

pub use builder::TreeBuilder;
pub use flags::NodeFlags;
pub use record::EntryRecord;
pub use tree::{DirAgg, NodeId, Tree};
pub use view::{NodeView, SortKey};
