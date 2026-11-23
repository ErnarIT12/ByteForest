//! Paged, sorted child lists and per-node details for the UI.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::category::{FileCategory, category_of};
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

/// What a search looks for.
#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchKind {
    /// Individual files.
    #[default]
    Files,
    /// Outermost folders, sized by their whole subtree.
    Folders,
}

/// Search filters over the entire tree.
#[derive(Deserialize, Debug, Default)]
pub struct FileFilter {
    /// Case-insensitive substring of the name.
    pub name: String,
    /// One extension (files only); merged with `extensions`.
    pub extension: String,
    /// Minimum size: a file's size, or a folder's total size.
    pub min_size: u64,
    /// Only files of this category (files only).
    #[serde(default)]
    pub category: Option<FileCategory>,
    /// Only nodes inside the subtree of this node (`NodeId.0`). An unknown or removed node
    /// matches nothing.
    #[serde(default)]
    pub under: Option<u32>,
    #[serde(default)]
    pub kind: SearchKind,
    /// Exact names, ASCII case-insensitive; empty means any.
    #[serde(default)]
    pub names: Vec<String>,
    /// Any-of extensions (files only), with or without a leading dot.
    #[serde(default)]
    pub extensions: Vec<String>,
}

/// Result of [`Tree::largest_files`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SearchOutcome {
    /// Number of matches.
    pub total: usize,
    /// Sum of the sizes of all matches, not only the returned ones.
    pub total_bytes: u64,
    /// The largest matches, biggest first.
    pub ids: Vec<NodeId>,
}

/// Keeps the `limit` largest matches and totals over all of them.
struct Collector {
    limit: usize,
    total: usize,
    total_bytes: u64,
    heap: BinaryHeap<Reverse<(u64, Reverse<u32>)>>,
}

impl Collector {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            total: 0,
            total_bytes: 0,
            heap: BinaryHeap::new(),
        }
    }

    fn push(&mut self, raw: u32, size: u64) {
        self.total += 1;
        self.total_bytes = self.total_bytes.saturating_add(size);
        let key = Reverse((size, Reverse(raw)));
        if self.heap.len() < self.limit {
            self.heap.push(key);
        } else if self.heap.peek().is_some_and(|smallest| key < *smallest) {
            *self.heap.peek_mut().expect("heap with an existing minimum") = key;
        }
    }

    fn finish(self) -> SearchOutcome {
        SearchOutcome {
            total: self.total,
            total_bytes: self.total_bytes,
            ids: self
                .heap
                .into_sorted_vec()
                .into_iter()
                .map(|Reverse((_, Reverse(raw)))| NodeId(raw))
                .collect(),
        }
    }
}

/// ASCII-case-insensitive substring test; `needle` must already be lowercase.
fn contains_lowercase(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.is_ascii() {
        // ASCII bytes never occur inside multi-byte UTF-8 sequences.
        let n = needle.as_bytes();
        return haystack
            .as_bytes()
            .windows(n.len())
            .any(|w| w.eq_ignore_ascii_case(n));
    }
    haystack.to_lowercase().contains(needle)
}

/// `ext` equals one of the lowercase `wanted` extensions.
fn extension_in(ext: &str, wanted: &[String]) -> bool {
    if ext.is_ascii() {
        wanted.iter().any(|w| ext.eq_ignore_ascii_case(w))
    } else {
        let lower = ext.to_lowercase();
        wanted.contains(&lower)
    }
}

impl Tree {
    /// Finds files (or outermost folders) matching `filter`: the exact count, the summed
    /// size of every match, and the `limit` largest.
    pub fn largest_files(&self, filter: &FileFilter, limit: usize) -> SearchOutcome {
        let name = filter.name.trim().to_lowercase();
        let mut extensions: Vec<String> = filter
            .extensions
            .iter()
            .chain(std::iter::once(&filter.extension))
            .map(|e| e.trim().trim_start_matches('.').to_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
        extensions.sort();
        extensions.dedup();
        let names: Vec<&str> = filter
            .names
            .iter()
            .map(|n| n.trim())
            .filter(|n| !n.is_empty())
            .collect();
        let name_ok = |filename: &str| {
            (names.is_empty() || names.iter().any(|n| n.eq_ignore_ascii_case(filename)))
                && contains_lowercase(filename, &name)
        };
        let mut out = Collector::new(limit);
        let under = match filter.under {
            None => None,
            Some(under) if (under as usize) < self.len() && !self.is_removed(NodeId(under)) => {
                Some(under)
            }
            Some(_) => return out.finish(),
        };
        if filter.kind == SearchKind::Folders {
            // Explicit stack: trees can be very deep. A match is not descended into, so only
            // the outermost match of a nested chain counts.
            let mut stack: Vec<u32> = match under {
                Some(under) => vec![under],
                None => self.children_raw(0).collect(),
            };
            while let Some(raw) = stack.pop() {
                let id = NodeId(raw);
                let flags = self.flags(id);
                if flags.contains(NodeFlags::REMOVED) || !flags.contains(NodeFlags::DIR) {
                    continue;
                }
                let size = self.total(id).size;
                if size >= filter.min_size && name_ok(self.name(id)) {
                    out.push(raw, size);
                } else {
                    stack.extend(self.children_raw(raw));
                }
            }
            return out.finish();
        }
        let mut consider = |raw: u32| {
            let id = NodeId(raw);
            let size = self.size[id.idx()];
            let flags = self.flags(id);
            if flags.intersects(NodeFlags::DIR | NodeFlags::REMOVED) || size < filter.min_size {
                return;
            }
            let filename = self.name(id);
            if filter
                .category
                .is_some_and(|c| category_of(filename, flags) != c)
            {
                return;
            }
            if !name_ok(filename) {
                return;
            }
            if !extensions.is_empty()
                && !Path::new(filename)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| extension_in(ext, &extensions))
            {
                return;
            }
            out.push(raw, size);
        };
        match under {
            None => (1..self.len() as u32).for_each(&mut consider),
            Some(under) => {
                let mut stack = vec![under];
                while let Some(n) = stack.pop() {
                    consider(n);
                    stack.extend(self.children_raw(n));
                }
            }
        }
        out.finish()
    }

    /// Raw ids of a node's children.
    fn children_raw(&self, raw: u32) -> impl Iterator<Item = u32> + '_ {
        let mut c = self.first_child[raw as usize];
        std::iter::from_fn(move || {
            if c == NONE {
                return None;
            }
            let current = c;
            c = self.next_sibling[current as usize];
            Some(current)
        })
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
