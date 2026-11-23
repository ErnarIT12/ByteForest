//! Paged, sorted child lists and per-node details for the UI.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::flags::NodeFlags;
use crate::tree::{NONE, NodeId, Tree};

/// Largest page `Tree::children` returns, whatever `limit` the caller asks for.
const MAX_PAGE: usize = 1000;

/// Order of a directory's children.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    /// Largest subtree first.
    SizeDesc,
    /// Case-insensitive name, A to Z.
    NameAsc,
    /// Most recently modified first.
    ModifiedDesc,
}

impl SortKey {
    /// Position of this key in the per-directory cache array.
    fn slot(self) -> usize {
        match self {
            SortKey::SizeDesc => 0,
            SortKey::NameAsc => 1,
            SortKey::ModifiedDesc => 2,
        }
    }
}

/// What the UI needs to know about one node.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct NodeView {
    /// `NodeId.0` of the node.
    pub id: u32,
    pub name: String,
    /// Logical size of the subtree in bytes (a file's own size).
    pub size: u64,
    /// Allocated size of the subtree in bytes.
    pub alloc: u64,
    /// Files in the subtree (`1` for a file).
    pub files: u64,
    /// Directories in the subtree, not counting the node itself.
    pub dirs: u64,
    pub is_dir: bool,
    pub has_children: bool,
    /// Modification time as a Windows FILETIME.
    pub mtime: i64,
    /// Share of the parent's size in percent; `0.0` for the root or an empty parent.
    pub percent_of_parent: f32,
}

/// Search filters over files in the entire tree.
#[derive(Deserialize, Debug, Default)]
pub struct FileFilter {
    pub name: String,
    pub extension: String,
    pub min_size: u64,
}

impl Tree {
    /// Returns the total number of matching files and a bounded list of the largest.
    pub fn largest_files(&self, filter: &FileFilter, limit: usize) -> (usize, Vec<NodeId>) {
        let name = filter.name.trim().to_lowercase();
        let extension = filter
            .extension
            .trim()
            .trim_start_matches('.')
            .to_lowercase();
        let mut matches = 0;
        let mut largest = BinaryHeap::new();
        for raw in 1..self.len() as u32 {
            let id = NodeId(raw);
            let size = self.size[id.idx()];
            if self.flags(id).contains(NodeFlags::DIR) || size < filter.min_size {
                continue;
            }
            let filename = self.name(id);
            if !name.is_empty() && !filename.to_lowercase().contains(&name) {
                continue;
            }
            if !extension.is_empty()
                && !Path::new(filename)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.to_lowercase() == extension)
            {
                continue;
            }
            matches += 1;
            let key = Reverse((size, Reverse(raw)));
            if largest.len() < limit {
                largest.push(key);
            } else if largest.peek().is_some_and(|smallest| key < *smallest) {
                *largest.peek_mut().expect("heap with an existing minimum") = key;
            }
        }
        let ids = largest
            .into_sorted_vec()
            .into_iter()
            .map(|Reverse((_, Reverse(raw)))| NodeId(raw))
            .collect();
        (matches, ids)
    }

    /// Zero-based position of a node in its parent's sorted child list.
    pub fn child_position(&self, id: NodeId, sort: SortKey) -> Option<usize> {
        let parent = self.parent(id)?;
        self.child_order(parent, sort)
            .binary_search_by(|&child| self.compare_children(NodeId(child), id, sort))
            .ok()
    }

    fn child_order(&self, dir: NodeId, sort: SortKey) -> &[u32] {
        let slot = self.dir_index[dir.idx()];
        self.sorted[slot as usize][sort.slot()].get_or_init(|| self.sort_children(dir, sort))
    }

    /// Details of one node.
    ///
    /// # Panics
    /// If `id` does not belong to this tree.
    pub fn view(&self, id: NodeId) -> NodeView {
        self.assert_in_range(id);
        let total = self.total(id);
        let percent_of_parent = match self.parent(id) {
            Some(p) => match self.total(p).size {
                0 => 0.0,
                parent_size => (100.0 * total.size as f64 / parent_size as f64) as f32,
            },
            None => 0.0,
        };
        NodeView {
            id: id.0,
            name: self.name(id).to_string(),
            size: total.size,
            alloc: total.alloc,
            files: total.files,
            dirs: total.dirs,
            is_dir: self.flags(id).contains(NodeFlags::DIR),
            has_children: self.first_child[id.idx()] != NONE,
            mtime: self.mtime(id),
            percent_of_parent,
        }
    }

    /// One page of the node's children in the given order.
    ///
    /// `limit` is capped at 1000. An `offset` past the end gives an empty vector. The sorted
    /// order of a directory is computed on first use and then kept for later calls.
    ///
    /// # Panics
    /// If `id` does not belong to this tree.
    pub fn children(
        &self,
        id: NodeId,
        sort: SortKey,
        offset: usize,
        limit: usize,
    ) -> Vec<NodeView> {
        self.assert_in_range(id);
        let slot = self.dir_index[id.idx()];
        if slot == NONE {
            return Vec::new(); // a file has no children
        }
        let order = self.child_order(id, sort);
        let start = offset.min(order.len());
        let end = start.saturating_add(limit.min(MAX_PAGE)).min(order.len());
        order[start..end]
            .iter()
            .map(|&c| self.view(NodeId(c)))
            .collect()
    }

    /// Number of direct children (`0` for files and empty directories).
    ///
    /// # Panics
    /// If `id` does not belong to this tree.
    pub fn child_count(&self, id: NodeId) -> usize {
        self.assert_in_range(id);
        self.children_ids(id).count()
    }

    fn assert_in_range(&self, id: NodeId) {
        assert!(
            id.idx() < self.len(),
            "NodeId({}) is out of range for a tree of {} nodes",
            id.0,
            self.len()
        );
    }

    /// Sorts the children of `id`. The order is total (ties fall back to name, then id), so
    /// the result does not depend on the sort algorithm.
    fn sort_children(&self, id: NodeId, sort: SortKey) -> Box<[u32]> {
        let mut ids: Vec<NodeId> = self.children_ids(id).collect();
        ids.sort_unstable_by(|&a, &b| self.compare_children(a, b, sort));
        ids.into_iter().map(|n| n.0).collect()
    }

    fn compare_children(&self, a: NodeId, b: NodeId, sort: SortKey) -> Ordering {
        let primary = match sort {
            SortKey::SizeDesc => self.total(b).size.cmp(&self.total(a).size),
            SortKey::NameAsc => Ordering::Equal,
            SortKey::ModifiedDesc => self.mtime(b).cmp(&self.mtime(a)),
        };
        primary
            .then_with(|| cmp_names_ignoring_case(self.name(a), self.name(b)))
            .then_with(|| a.0.cmp(&b.0))
    }
}

/// Compares two names character by character after lowercasing, without allocating.
fn cmp_names_ignoring_case(a: &str, b: &str) -> Ordering {
    a.chars()
        .flat_map(char::to_lowercase)
        .cmp(b.chars().flat_map(char::to_lowercase))
}
