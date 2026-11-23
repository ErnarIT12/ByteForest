//! Comparing a current tree with an older one (a loaded snapshot).
//!
//! Entries are matched by their chain of names starting at the roots; the roots always match,
//! whatever they are called. Ids and paths of the two trees are never compared, so the result
//! does not depend on how either tree was numbered.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use crate::flags::NodeFlags;
use crate::tree::{NodeId, Tree};

/// Marks a node in `base_size` that does not exist in the base tree.
const ADDED: u64 = u64::MAX;

/// Bytes and entry count of something that is no longer there (or is new).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Gone {
    /// Logical bytes.
    pub bytes: u64,
    /// Number of files and directories, directories included.
    pub count: u64,
}

impl Gone {
    fn add(&mut self, other: Gone) {
        self.bytes = self.bytes.saturating_add(other.bytes);
        self.count = self.count.saturating_add(other.count);
    }
}

/// How a node differs from the base tree.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DiffStatus {
    /// Not present in the base tree.
    Added,
    /// Bigger than in the base tree.
    Grown,
    /// Smaller than in the base tree.
    Shrunk,
    /// Same size as in the base tree.
    Unchanged,
}

/// The comparison result for one node of the current tree.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DiffEntry {
    /// Kind of change.
    pub status: DiffStatus,
    /// Size in the base tree, `None` for added nodes.
    pub base: Option<u64>,
    /// Size in the current tree.
    pub current: u64,
    /// `current - base` (the whole size for added nodes), saturated to the `i64` range.
    pub delta: i64,
}

/// Result of [`diff`], aligned to the node ids of the current tree.
#[derive(Clone, Debug)]
pub struct SnapshotDiff {
    /// Per current node: subtree size of the matching base node, or [`ADDED`].
    base_size: Vec<u64>,
    /// Per current directory that contains something that is gone from the base tree: the
    /// total of everything gone anywhere below it (the directory's own subtree).
    removed: HashMap<u32, Gone>,
    /// Top-most added entries.
    added: Gone,
    base_total: u64,
    current_total: u64,
}

/// Totals of a comparison.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DiffSummary {
    /// Size of the whole base tree.
    pub base_total: u64,
    /// Size of the whole current tree.
    pub current_total: u64,
    /// `current_total - base_total`, saturated.
    pub total_delta: i64,
    /// Everything that exists now but not in the base tree.
    pub added: Gone,
    /// Everything that existed in the base tree but is gone.
    pub removed: Gone,
}

fn clamp_delta(current: u64, base: u64) -> i64 {
    let d = i128::from(current) - i128::from(base);
    i64::try_from(d).unwrap_or(if d < 0 { i64::MIN } else { i64::MAX })
}

fn is_dir(t: &Tree, id: NodeId) -> bool {
    t.flags(id).contains(NodeFlags::DIR)
}

/// Number of entries in the subtree of `id`, itself included.
fn entries_in(t: &Tree, id: NodeId) -> u64 {
    let agg = t.total(id);
    if is_dir(t, id) {
        agg.files.saturating_add(agg.dirs).saturating_add(1)
    } else {
        1
    }
}

fn gone_of(t: &Tree, id: NodeId) -> Gone {
    Gone {
        bytes: t.total(id).size,
        count: entries_in(t, id),
    }
}

/// Compares `current` with `base`.
///
/// Children are matched by name inside matched directories; with `case_insensitive` names are
/// compared lowercased (use it for file systems that ignore case). A file never matches a
/// directory of the same name. Each base child matches at most one current child.
///
/// Cost: every directory present in both trees is visited once and each of its children is
/// hashed once, so time and extra memory are O(n) in the number of nodes.
pub fn diff(base: &Tree, current: &Tree, case_insensitive: bool) -> SnapshotDiff {
    let key = |name: &str| {
        if case_insensitive {
            name.to_lowercase()
        } else {
            name.to_string()
        }
    };

    let mut base_size = vec![ADDED; current.len()];
    let mut removed: HashMap<u32, Gone> = HashMap::new();
    let mut added = Gone::default();
    let mut visited: Vec<NodeId> = Vec::new();

    let (base_root, cur_root) = (base.root(), current.root());
    base_size[cur_root.0 as usize] = base.total(base_root).size;
    let mut stack = vec![(base_root, cur_root)];

    while let Some((b, c)) = stack.pop() {
        visited.push(c);
        let mut by_name: HashMap<String, NodeId> = HashMap::new();
        let mut leftovers: Vec<NodeId> = Vec::new();
        for bc in base.children_ids(b) {
            if base.is_removed(bc) {
                continue;
            }
            match by_name.entry(key(base.name(bc))) {
                Entry::Vacant(slot) => {
                    slot.insert(bc);
                }
                // Name collision: only the first child can match.
                Entry::Occupied(_) => leftovers.push(bc),
            }
        }
        for cc in current.children_ids(c) {
            if current.is_removed(cc) {
                continue;
            }
            let k = key(current.name(cc));
            let matched = by_name
                .get(&k)
                .copied()
                .filter(|&bc| is_dir(base, bc) == is_dir(current, cc));
            match matched {
                Some(bc) => {
                    by_name.remove(&k);
                    base_size[cc.0 as usize] = base.total(bc).size;
                    if is_dir(current, cc) {
                        stack.push((bc, cc));
                    }
                }
                None => added.add(gone_of(current, cc)),
            }
        }
        let mut gone = Gone::default();
        for bc in by_name.into_values().chain(leftovers) {
            gone.add(gone_of(base, bc));
        }
        if gone.count > 0 {
            removed.entry(c.0).or_default().add(gone);
        }
    }

    // Fold every directory's removals into its parent. `visited` lists parents before their
    // children, so walking it backwards finishes a directory before its parent reads it.
    for &c in visited.iter().rev() {
        let Some(p) = current.parent(c) else { continue };
        if let Some(g) = removed.get(&c.0).copied() {
            removed.entry(p.0).or_default().add(g);
        }
    }

    SnapshotDiff {
        base_size,
        removed,
        added,
        base_total: base.total(base_root).size,
        current_total: current.total(cur_root).size,
    }
}

impl SnapshotDiff {
    /// Totals of the whole comparison.
    pub fn summary(&self) -> DiffSummary {
        DiffSummary {
            base_total: self.base_total,
            current_total: self.current_total,
            total_delta: clamp_delta(self.current_total, self.base_total),
            added: self.added,
            removed: self.removed.get(&0).copied().unwrap_or_default(),
        }
    }

    /// Comparison result for a node of `current` (the tree this diff was computed for).
    /// Unknown ids yield an unchanged entry of size 0.
    pub fn entry(&self, current: &Tree, id: NodeId) -> DiffEntry {
        let idx = id.0 as usize;
        let (Some(&base), true) = (self.base_size.get(idx), idx < current.len()) else {
            return DiffEntry {
                status: DiffStatus::Unchanged,
                base: Some(0),
                current: 0,
                delta: 0,
            };
        };
        let size = current.total(id).size;
        if base == ADDED {
            return DiffEntry {
                status: DiffStatus::Added,
                base: None,
                current: size,
                delta: clamp_delta(size, 0),
            };
        }
        let delta = clamp_delta(size, base);
        let status = match delta {
            0 => DiffStatus::Unchanged,
            d if d > 0 => DiffStatus::Grown,
            _ => DiffStatus::Shrunk,
        };
        DiffEntry {
            status,
            base: Some(base),
            current: size,
            delta,
        }
    }

    /// Everything inside the directory `id` that existed in the base tree but is gone now,
    /// nested directories included. Zero for unknown ids and for files.
    pub fn removed_under(&self, id: NodeId) -> Gone {
        self.removed.get(&id.0).copied().unwrap_or_default()
    }

    /// The nodes that grew or are new, largest increase first (ties by node id), at most
    /// `limit`. Files and folders are both listed. A folder is left out when one single
    /// descendant already accounts for at least 99% of its increase, so a big new file is
    /// not followed by every folder above it; the deepest entry explaining the growth is
    /// kept. The root and removed nodes are left out, as are nodes inside an added folder
    /// (the folder is listed) and added entries of size 0.
    pub fn top_changes(&self, current: &Tree, limit: usize) -> Vec<(NodeId, DiffEntry)> {
        let n = current.len();
        if limit == 0 || self.base_size.len() != n {
            return Vec::new();
        }
        let live = |i: usize| !current.is_removed(NodeId(i as u32));
        let mut out: Vec<(NodeId, DiffEntry)> = Vec::new();
        for i in 1..n {
            if !live(i) {
                continue;
            }
            let id = NodeId(i as u32);
            let e = self.entry(current, id);
            if e.delta <= 0 {
                continue;
            }
            let inside_added = e.status == DiffStatus::Added
                && current
                    .parent(id)
                    .is_some_and(|p| self.base_size[p.0 as usize] == ADDED);
            if !inside_added {
                out.push((id, e));
            }
        }

        // Drop every ancestor whose growth is (nearly) all due to one listed descendant.
        let mut hidden = vec![false; n];
        for &(id, e) in &out {
            let mut cur = current.parent(id);
            while let Some(p) = cur {
                if p.0 == 0 {
                    break;
                }
                let pd = self.entry(current, p).delta;
                if pd > 0 && i128::from(e.delta) * 100 >= i128::from(pd) * 99 {
                    hidden[p.0 as usize] = true;
                }
                cur = current.parent(p);
            }
        }
        out.retain(|(id, _)| !hidden[id.0 as usize]);
        out.sort_unstable_by(|a, b| b.1.delta.cmp(&a.1.delta).then(a.0.0.cmp(&b.0.0)));
        out.truncate(limit);
        out
    }
}
