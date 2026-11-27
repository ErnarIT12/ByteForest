//! Duplicate files, the part that needs no disk access: which files could be copies of each
//! other (same exact size), and how confirmed groups follow later changes of the tree.
//!
//! Reading and hashing file contents happens in `bf-scan`; this module only works on the
//! in-memory [`Tree`], so it stays free of platform APIs.

use serde::Deserialize;

use crate::category::{FileCategory, category_of};
use crate::flags::NodeFlags;
use crate::tree::{NodeId, Tree};

/// Smallest file size checked by default (1 MiB).
pub const DEFAULT_DUPLICATE_MIN_SIZE: u64 = 1 << 20;

/// Which files take part in a duplicate search.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct DuplicateFilter {
    /// Smallest file size in bytes. Empty files never count, whatever this says.
    pub min_size: u64,
    /// Only files inside the subtree of this node (`NodeId.0`). An unknown or removed node
    /// matches nothing.
    pub under: Option<u32>,
    /// Only files of this category.
    pub category: Option<FileCategory>,
    /// Case-insensitive substring of the file name; empty means any.
    pub name: String,
    /// Also check hidden and system files, dot-files, and everything inside hidden or system
    /// folders.
    pub include_hidden: bool,
}

impl Default for DuplicateFilter {
    fn default() -> Self {
        Self {
            min_size: DEFAULT_DUPLICATE_MIN_SIZE,
            under: None,
            category: None,
            name: String::new(),
            include_hidden: false,
        }
    }
}

/// Files that share one exact size: possible copies of each other.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SizeGroup {
    /// Size of every file in the group, in bytes.
    pub size: u64,
    /// At least two files, in ascending id order.
    pub ids: Vec<NodeId>,
}

/// Result of [`Tree::duplicate_candidates`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DuplicateCandidates {
    /// Groups of two or more files with the same size, largest size first.
    pub groups: Vec<SizeGroup>,
    /// Number of files in all groups: what a content check has to read.
    pub files: u64,
    /// Summed size of all those files.
    pub bytes: u64,
}

/// Files with identical contents, confirmed by hashing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateGroup {
    /// Size of every copy, in bytes.
    pub size: u64,
    /// Content hash shared by every copy.
    pub hash: [u8; 32],
    /// At least two copies, in ascending id order.
    pub ids: Vec<NodeId>,
}

impl DuplicateGroup {
    /// Bytes freed by keeping one copy and removing the others.
    pub fn reclaimable(&self) -> u64 {
        self.size
            .saturating_mul(self.ids.len().saturating_sub(1) as u64)
    }
}

/// Sorts groups by reclaimable space (largest first), then by size and hash, so the order is
/// the same on every run.
pub fn sort_duplicate_groups(groups: &mut [DuplicateGroup]) {
    groups.sort_unstable_by(|a, b| {
        b.reclaimable()
            .cmp(&a.reclaimable())
            .then_with(|| b.size.cmp(&a.size))
            .then_with(|| a.hash.cmp(&b.hash))
            .then_with(|| a.ids.iter().map(|n| n.0).cmp(b.ids.iter().map(|n| n.0)))
    });
}

/// Hidden for a duplicate search: hidden or system attribute, or a dot-file such as `.git`.
fn looks_hidden(name: &str, flags: NodeFlags) -> bool {
    flags.intersects(NodeFlags::HIDDEN | NodeFlags::SYSTEM) || name.starts_with('.')
}

/// Flags that never make a file worth comparing: links share their data with another entry
/// and synthetic or removed nodes are not on the disk.
const NEVER: NodeFlags = NodeFlags::DIR
    .union(NodeFlags::HARDLINK)
    .union(NodeFlags::REPARSE)
    .union(NodeFlags::SYNTHETIC)
    .union(NodeFlags::REMOVED);

impl Tree {
    /// Groups the files matching `filter` by exact size, without reading the disk.
    ///
    /// Hard links, reparse points (symlinks, junctions), synthetic and removed nodes and empty
    /// files are always left out. Unless `filter.include_hidden` is set, so are hidden and
    /// system files, dot-files and the contents of hidden or system folders. Folder reparse
    /// points are never entered.
    pub fn duplicate_candidates(&self, filter: &DuplicateFilter) -> DuplicateCandidates {
        let start = match filter.under {
            None => self.root(),
            Some(raw) if (raw as usize) < self.len() && !self.is_removed(NodeId(raw)) => {
                NodeId(raw)
            }
            Some(_) => return DuplicateCandidates::default(),
        };
        let min_size = filter.min_size.max(1);
        let needle = filter.name.trim().to_lowercase();
        let name_ok = |name: &str| {
            needle.is_empty()
                || if needle.is_ascii() {
                    name.as_bytes()
                        .windows(needle.len())
                        .any(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
                } else {
                    name.to_lowercase().contains(&needle)
                }
        };

        let mut found: Vec<(u64, NodeId)> = Vec::new();
        // Explicit stack: trees can be very deep.
        let mut stack = vec![start];
        while let Some(id) = stack.pop() {
            let flags = self.flags(id);
            if flags.contains(NodeFlags::REMOVED) {
                continue;
            }
            let name = self.name(id);
            // The scanned root is never skipped for its own name or attributes.
            let skip_hidden =
                !filter.include_hidden && id != self.root() && looks_hidden(name, flags);
            if flags.contains(NodeFlags::DIR) {
                if !flags.contains(NodeFlags::REPARSE) && !skip_hidden {
                    stack.extend(self.children_ids(id));
                }
                continue;
            }
            if flags.intersects(NEVER) || skip_hidden {
                continue;
            }
            let size = self.total(id).size;
            if size < min_size || !name_ok(name) {
                continue;
            }
            if filter
                .category
                .is_some_and(|c| category_of(name, flags) != c)
            {
                continue;
            }
            found.push((size, id));
        }

        // Largest size first; ids ascending inside a size.
        found.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.0.cmp(&b.1.0)));
        let mut out = DuplicateCandidates::default();
        for run in found.chunk_by(|a, b| a.0 == b.0) {
            if run.len() < 2 {
                continue;
            }
            let size = run[0].0;
            out.files += run.len() as u64;
            out.bytes = out
                .bytes
                .saturating_add(size.saturating_mul(run.len() as u64));
            out.groups.push(SizeGroup {
                size,
                ids: run.iter().map(|&(_, id)| id).collect(),
            });
        }
        out
    }

    /// The groups with copies that are no longer in the tree dropped (for example after
    /// they were moved to the trash), and groups left with fewer than two copies removed.
    /// Ids outside the tree are dropped too. The order of `groups` is kept.
    pub fn prune_duplicate_groups(&self, groups: &[DuplicateGroup]) -> Vec<DuplicateGroup> {
        groups
            .iter()
            .filter_map(|group| {
                let ids: Vec<NodeId> = group
                    .ids
                    .iter()
                    .copied()
                    .filter(|&id| id.idx() < self.len() && !self.is_removed(id))
                    .collect();
                (ids.len() >= 2).then_some(DuplicateGroup {
                    size: group.size,
                    hash: group.hash,
                    ids,
                })
            })
            .collect()
    }
}
