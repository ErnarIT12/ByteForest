//! Removing a subtree from a finished tree (e.g. after the user moved it to the trash).

use std::fmt;
use std::sync::OnceLock;

use crate::flags::NodeFlags;
use crate::tree::{NONE, NodeId, Tree};

/// Totals of a removed subtree; a directory counts itself in `dirs`, a file in `files`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Removed {
    /// Logical bytes.
    pub bytes: u64,
    /// Allocated bytes.
    pub alloc: u64,
    /// Files removed.
    pub files: u64,
    /// Directories removed, including the node itself when it is a directory.
    pub dirs: u64,
}

/// Why [`Tree::remove_subtree`] refused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RemoveError {
    /// The id does not belong to this tree.
    OutOfRange,
    /// The root cannot be removed.
    Root,
    /// The node was removed before.
    AlreadyRemoved,
    /// The parent links do not lead to the root.
    BrokenParentChain,
}

impl fmt::Display for RemoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RemoveError::OutOfRange => "node is out of range",
            RemoveError::Root => "the root cannot be removed",
            RemoveError::AlreadyRemoved => "node was already removed",
            RemoveError::BrokenParentChain => "node has a broken parent chain",
        })
    }
}

impl std::error::Error for RemoveError {}

impl Tree {
    /// Whether the node was removed with [`Tree::remove_subtree`]. `false` for unknown ids.
    pub fn is_removed(&self, id: NodeId) -> bool {
        self.flags
            .get(id.idx())
            .is_some_and(|f| f.contains(NodeFlags::REMOVED))
    }

    /// Unlinks the node from its parent, flags it and all descendants as removed and
    /// subtracts its totals from every ancestor. Sort caches of the ancestors are reset.
    ///
    /// The tree is not modified when an error is returned.
    pub fn remove_subtree(&mut self, id: NodeId) -> Result<Removed, RemoveError> {
        let idx = id.idx();
        if idx >= self.len() {
            return Err(RemoveError::OutOfRange);
        }
        if idx == 0 {
            return Err(RemoveError::Root);
        }
        if self.is_removed(id) {
            return Err(RemoveError::AlreadyRemoved);
        }

        // Validate the ancestor chain up to the root before changing anything.
        let mut ancestors = Vec::new();
        let mut cur = self.parent[idx];
        while cur != NONE {
            if ancestors.len() >= self.len() || cur as usize >= self.len() {
                return Err(RemoveError::BrokenParentChain);
            }
            if self.dir_index[cur as usize] == NONE || self.is_removed(NodeId(cur)) {
                return Err(RemoveError::BrokenParentChain);
            }
            ancestors.push(cur);
            cur = self.parent[cur as usize];
        }
        let parent = match ancestors.first() {
            Some(&p) if ancestors.last() == Some(&0) => p as usize,
            _ => return Err(RemoveError::BrokenParentChain),
        };

        // Unlink from the sibling list.
        let next = self.next_sibling[idx];
        if self.first_child[parent] == id.0 {
            self.first_child[parent] = next;
        } else {
            let mut sib = self.first_child[parent];
            let mut steps = 0usize;
            while sib != NONE && self.next_sibling[sib as usize] != id.0 {
                sib = self.next_sibling[sib as usize];
                steps += 1;
                if steps > self.len() {
                    return Err(RemoveError::BrokenParentChain);
                }
            }
            if sib == NONE {
                return Err(RemoveError::BrokenParentChain);
            }
            self.next_sibling[sib as usize] = next;
        }
        self.next_sibling[idx] = NONE;

        // Totals as the ancestors counted them.
        let total = self.total(id);
        let removed = Removed {
            bytes: total.size,
            alloc: total.alloc,
            files: total.files,
            dirs: if self.dir_index[idx] == NONE {
                0
            } else {
                total.dirs.saturating_add(1)
            },
        };

        // Flag the subtree.
        let mut stack = vec![id.0];
        while let Some(n) = stack.pop() {
            self.flags[n as usize].insert(NodeFlags::REMOVED);
            let mut c = self.first_child[n as usize];
            while c != NONE {
                stack.push(c);
                c = self.next_sibling[c as usize];
            }
        }

        // Update ancestors and reset their sort caches.
        for &a in &ancestors {
            let slot = self.dir_index[a as usize] as usize;
            let agg = &mut self.aggs[slot];
            agg.size = agg.size.saturating_sub(removed.bytes);
            agg.alloc = agg.alloc.saturating_sub(removed.alloc);
            agg.files = agg.files.saturating_sub(removed.files);
            agg.dirs = agg.dirs.saturating_sub(removed.dirs);
            self.sorted[slot] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
        }
        Ok(removed)
    }
}
