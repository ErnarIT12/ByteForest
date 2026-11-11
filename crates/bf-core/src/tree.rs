use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::flags::NodeFlags;

/// Marks "no node" / "not a directory" in the `u32` index columns.
pub(crate) const NONE: u32 = u32::MAX;

/// Index of a node inside a [`Tree`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct NodeId(pub u32);

impl NodeId {
    pub(crate) fn idx(self) -> usize {
        self.0 as usize
    }
}

/// Totals for a directory subtree (the directory's own size is included).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct DirAgg {
    /// Sum of logical sizes in bytes.
    pub size: u64,
    /// Sum of allocated sizes in bytes.
    pub alloc: u64,
    /// Number of files in the subtree.
    pub files: u64,
    /// Number of directories in the subtree, not counting the directory itself.
    pub dirs: u64,
}

/// Immutable file tree in structure-of-arrays layout.
///
/// All columns are indexed by `NodeId.0`. Accessors panic if given a `NodeId` that did not
/// come from this tree.
#[derive(Debug)]
pub struct Tree {
    pub(crate) parent: Vec<u32>,
    pub(crate) name_off: Vec<u32>,
    pub(crate) name_len: Vec<u32>,
    pub(crate) size: Vec<u64>,
    pub(crate) alloc: Vec<u64>,
    pub(crate) mtime: Vec<i64>,
    pub(crate) flags: Vec<NodeFlags>,
    pub(crate) first_child: Vec<u32>,
    pub(crate) next_sibling: Vec<u32>,
    /// All names back to back; `name_off`/`name_len` slice into it.
    pub(crate) names: String,
    /// Maps a node to its slot in `aggs`, or [`NONE`] for files.
    pub(crate) dir_index: Vec<u32>,
    pub(crate) aggs: Vec<DirAgg>,
    /// Lazily built sorted child lists, one `[OnceLock; 3]` per directory slot (same index as
    /// `aggs`), one lock per [`crate::SortKey`]. Filled by `Tree::children`.
    pub(crate) sorted: Vec<[OnceLock<Box<[u32]>>; 3]>,
}

impl Tree {
    /// The root node (always present, always a directory).
    pub fn root(&self) -> NodeId {
        NodeId(0)
    }

    /// Number of nodes, including the root.
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    /// Always `false`: a tree has at least its root.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Name of the node (the root's name is its volume label such as `C:`).
    pub fn name(&self, id: NodeId) -> &str {
        let off = self.name_off[id.idx()] as usize;
        let len = self.name_len[id.idx()] as usize;
        &self.names[off..off + len]
    }

    /// Parent of the node, `None` for the root.
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        match self.parent[id.idx()] {
            NONE => None,
            p => Some(NodeId(p)),
        }
    }

    /// Attribute bits of the node.
    pub fn flags(&self, id: NodeId) -> NodeFlags {
        self.flags[id.idx()]
    }

    /// Modification time of the node as a Windows FILETIME.
    pub fn mtime(&self, id: NodeId) -> i64 {
        self.mtime[id.idx()]
    }

    /// Subtree totals. For a file: its own size and allocation, `files = 1`, `dirs = 0`.
    pub fn total(&self, id: NodeId) -> DirAgg {
        match self.dir_index[id.idx()] {
            NONE => DirAgg {
                size: self.size[id.idx()],
                alloc: self.alloc[id.idx()],
                files: 1,
                dirs: 0,
            },
            slot => self.aggs[slot as usize],
        }
    }

    /// Full path joined with `\`; the root contributes just its name (`C:`).
    pub fn path_of(&self, id: NodeId) -> String {
        let mut parts = Vec::new();
        let mut cur = Some(id);
        while let Some(n) = cur {
            parts.push(self.name(n));
            cur = self.parent(n);
        }
        parts.reverse();
        parts.join("\\")
    }

    /// Direct children in insertion order.
    pub fn children_ids(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let first = self.first_child[id.idx()];
        std::iter::successors((first != NONE).then_some(first), |&c| {
            let next = self.next_sibling[c as usize];
            (next != NONE).then_some(next)
        })
        .map(NodeId)
    }
}
