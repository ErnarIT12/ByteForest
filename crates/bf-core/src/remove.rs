//! Removing a subtree from a finished tree (e.g. after the user moved it to the trash).

use std::collections::HashSet;
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

        // Collect the subtree before anything changes. The walk is bounded by the node count
        // so cyclic or out-of-range child links in a corrupt tree end in an error.
        let subtree = self.collect_subtree(id.0)?;

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
        for &n in &subtree {
            self.flags[n as usize].insert(NodeFlags::REMOVED);
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

    /// The outermost of `ids`: drops ids that are out of range, removed, the root or repeated,
    /// and every id with an ancestor that is also kept. The order of the survivors follows
    /// `ids`. Each ancestor walk is bounded by the node count, so corrupt parent links end
    /// the walk instead of looping.
    pub fn outermost(&self, ids: &[NodeId]) -> Vec<NodeId> {
        let mut candidates = HashSet::with_capacity(ids.len());
        let valid: Vec<NodeId> = ids
            .iter()
            .copied()
            .filter(|&id| id.idx() != 0 && id.idx() < self.len() && !self.is_removed(id))
            .filter(|&id| candidates.insert(id.0))
            .collect();
        valid
            .into_iter()
            .filter(|&id| {
                let mut cur = self.parent[id.idx()];
                let mut steps = 0usize;
                while cur != NONE && (cur as usize) < self.len() && steps <= self.len() {
                    if candidates.contains(&cur) {
                        return false;
                    }
                    cur = self.parent[cur as usize];
                    steps += 1;
                }
                true
            })
            .collect()
    }

    /// Ids of `root` and all its descendants, using an explicit stack and at most
    /// `self.len()` steps. Fails when a link is out of range or the walk does not end.
    fn collect_subtree(&self, root: u32) -> Result<Vec<u32>, RemoveError> {
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if out.len() >= self.len() || n as usize >= self.len() {
                return Err(RemoveError::BrokenParentChain);
            }
            out.push(n);
            let mut c = self.first_child[n as usize];
            let mut siblings = 0usize;
            while c != NONE {
                if c as usize >= self.len() || siblings >= self.len() {
                    return Err(RemoveError::BrokenParentChain);
                }
                stack.push(c);
                siblings += 1;
                c = self.next_sibling[c as usize];
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::TreeBuilder;
    use crate::record::EntryRecord;

    fn rec(key: u64, parent: u64, name: &str, size: u64, dir: bool) -> EntryRecord {
        EntryRecord {
            key,
            parent_key: parent,
            name: name.to_string(),
            size,
            alloc: size,
            mtime: 0,
            flags: if dir {
                NodeFlags::DIR
            } else {
                NodeFlags::empty()
            },
        }
    }

    /// root -> d -> { f1, sub -> f2 }
    fn tree() -> Tree {
        let mut b = TreeBuilder::new(1, "root");
        b.push(rec(2, 1, "d", 0, true));
        b.push(rec(3, 2, "f1", 10, false));
        b.push(rec(4, 2, "sub", 0, true));
        b.push(rec(5, 4, "f2", 20, false));
        b.finish()
    }

    fn snapshot(t: &Tree) -> (Vec<NodeFlags>, Vec<u32>, Vec<u32>, Vec<crate::tree::DirAgg>) {
        (
            t.flags.clone(),
            t.first_child.clone(),
            t.next_sibling.clone(),
            t.aggs.clone(),
        )
    }

    #[test]
    fn cyclic_child_links_fail_without_modifying_the_tree() {
        let mut t = tree();
        // Make `sub` (id 3) list `d` (id 1) as a child: a cycle below the removed node.
        let sub = t
            .children_ids(NodeId(1))
            .find(|&c| t.name(c) == "sub")
            .unwrap();
        t.first_child[sub.idx()] = 1;
        let before = snapshot(&t);
        assert_eq!(
            t.remove_subtree(NodeId(1)),
            Err(RemoveError::BrokenParentChain)
        );
        assert_eq!(snapshot(&t), before);
    }

    #[test]
    fn out_of_range_child_link_fails_without_modifying_the_tree() {
        let mut t = tree();
        t.first_child[1] = 9999;
        let before = snapshot(&t);
        assert_eq!(
            t.remove_subtree(NodeId(1)),
            Err(RemoveError::BrokenParentChain)
        );
        assert_eq!(snapshot(&t), before);
    }

    #[test]
    fn valid_subtree_is_collected_completely() {
        let t = tree();
        let mut ids = t.collect_subtree(1).unwrap();
        ids.sort_unstable();
        assert_eq!(ids, [1, 2, 3, 4]);
    }
}
