use std::collections::HashMap;

use crate::flags::NodeFlags;
use crate::record::EntryRecord;
use crate::tree::{DirAgg, NONE, Tree};

/// Name of the synthetic directory that collects unreachable entries.
const ORPHANED_NAME: &str = "$Orphaned";
/// Key of the `$Orphaned` node; scanners never produce this value.
const ORPHANED_KEY: u64 = u64::MAX;

/// Collects [`EntryRecord`]s in any order and links them into a [`Tree`].
///
/// Node 0 is the root. Entries whose parent is missing, is not a directory, or that sit in
/// a parent cycle are moved under a synthetic `$Orphaned` directory below the root.
pub struct TreeBuilder {
    root_key: u64,
    /// Entry key -> node index; used to resolve `parent_key` and to drop duplicates.
    index_of_key: HashMap<u64, u32>,
    parent_key: Vec<u64>,
    name_off: Vec<u32>,
    name_len: Vec<u32>,
    size: Vec<u64>,
    alloc: Vec<u64>,
    mtime: Vec<i64>,
    flags: Vec<NodeFlags>,
    names: String,
}

impl TreeBuilder {
    /// Creates a builder whose root directory has the given key and name (e.g. `"C:"`).
    pub fn new(root_key: u64, root_name: &str) -> Self {
        let mut b = Self {
            root_key,
            index_of_key: HashMap::new(),
            parent_key: Vec::new(),
            name_off: Vec::new(),
            name_len: Vec::new(),
            size: Vec::new(),
            alloc: Vec::new(),
            mtime: Vec::new(),
            flags: Vec::new(),
            names: String::new(),
        };
        let root = EntryRecord {
            key: root_key,
            parent_key: root_key,
            name: root_name.to_string(),
            size: 0,
            alloc: 0,
            mtime: 0,
            flags: NodeFlags::DIR,
        };
        b.add_node(&root);
        b
    }

    /// Adds an entry. Records may arrive in any order (a child may precede its parent).
    ///
    /// A record whose key was already pushed is ignored (the first one wins), as is a record
    /// with the root's key; the root is created by [`TreeBuilder::new`].
    pub fn push(&mut self, rec: EntryRecord) {
        if self.index_of_key.contains_key(&rec.key) {
            return;
        }
        // Node ids and name offsets are u32; stop accepting data instead of overflowing.
        let fits = self.len() < (u32::MAX - 2) as usize
            && self.names.len().saturating_add(rec.name.len()) < u32::MAX as usize;
        if !fits {
            return;
        }
        self.add_node(&rec);
    }

    /// Number of nodes so far, including the root.
    pub fn len(&self) -> usize {
        self.parent_key.len()
    }

    /// Always `false`: the root exists from the start.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Appends a node. The caller has checked that the key is new and the limits hold.
    fn add_node(&mut self, rec: &EntryRecord) {
        let idx = self.parent_key.len() as u32;
        self.index_of_key.insert(rec.key, idx);
        self.parent_key.push(rec.parent_key);
        self.name_off.push(self.names.len() as u32);
        self.name_len.push(rec.name.len() as u32);
        self.names.push_str(&rec.name);
        self.size.push(rec.size);
        self.alloc.push(rec.alloc);
        self.mtime.push(rec.mtime);
        self.flags.push(rec.flags);
    }

    /// Resolves parents, moves unreachable nodes under `$Orphaned`, links children and
    /// computes directory aggregates.
    pub fn finish(mut self) -> Tree {
        let mut parent = self.resolve_parents();
        self.attach_unreachable(&mut parent);

        let n = self.len();
        let (first_child, next_sibling) = link_children(&parent);
        let order = bfs_order(&first_child, &next_sibling);
        debug_assert_eq!(
            order.len(),
            n,
            "every node must be reachable after orphan handling"
        );

        // Dense per-directory aggregate slots, seeded with each directory's own size.
        let mut dir_index = vec![NONE; n];
        let mut aggs = Vec::new();
        for (i, flags) in self.flags.iter().enumerate() {
            if flags.contains(NodeFlags::DIR) {
                dir_index[i] = aggs.len() as u32;
                aggs.push(DirAgg {
                    size: self.size[i],
                    alloc: self.alloc[i],
                    files: 0,
                    dirs: 0,
                });
            }
        }

        // Children come after their parent in BFS order, so walking backwards finishes every
        // child before its parent. Saturating adds: sizes come from untrusted disk data.
        for &v in order.iter().rev() {
            let v = v as usize;
            let p = parent[v];
            if p == NONE {
                continue;
            }
            let contribution = match dir_index[v] {
                NONE => DirAgg {
                    size: self.size[v],
                    alloc: self.alloc[v],
                    files: 1,
                    dirs: 0,
                },
                slot => {
                    let a = aggs[slot as usize];
                    DirAgg {
                        dirs: a.dirs.saturating_add(1),
                        ..a
                    }
                }
            };
            let parent_agg = &mut aggs[dir_index[p as usize] as usize];
            parent_agg.size = parent_agg.size.saturating_add(contribution.size);
            parent_agg.alloc = parent_agg.alloc.saturating_add(contribution.alloc);
            parent_agg.files = parent_agg.files.saturating_add(contribution.files);
            parent_agg.dirs = parent_agg.dirs.saturating_add(contribution.dirs);
        }

        Tree {
            parent,
            name_off: self.name_off,
            name_len: self.name_len,
            size: self.size,
            alloc: self.alloc,
            mtime: self.mtime,
            flags: self.flags,
            first_child,
            next_sibling,
            names: self.names,
            dir_index,
            aggs,
        }
    }

    /// Maps every `parent_key` to a node index. The root and nodes whose parent is missing or
    /// is not a directory get [`NONE`].
    fn resolve_parents(&self) -> Vec<u32> {
        (0..self.len())
            .map(|i| {
                if i == 0 {
                    return NONE;
                }
                match self.index_of_key.get(&self.parent_key[i]) {
                    Some(&p) if self.flags[p as usize].contains(NodeFlags::DIR) => p,
                    _ => NONE,
                }
            })
            .collect()
    }

    /// Makes every node reachable from the root by re-parenting the "top" of each detached
    /// chain (a node without a parent, or one node of a parent cycle) under `$Orphaned`.
    fn attach_unreachable(&mut self, parent: &mut Vec<u32>) {
        const UNKNOWN: u8 = 0;
        const IN_PATH: u8 = 1;
        const REACHABLE: u8 = 2;

        let mut state = vec![UNKNOWN; self.len()];
        state[0] = REACHABLE;
        let mut orphaned: Option<u32> = None;
        let mut path: Vec<u32> = Vec::new();

        for start in 1..state.len() {
            if state[start] != UNKNOWN {
                continue;
            }
            // Follow parent links upwards until we hit something already classified.
            path.clear();
            let mut cur = start as u32;
            let detached_top = loop {
                match state[cur as usize] {
                    REACHABLE => break None,
                    IN_PATH => break Some(cur), // found a cycle; break it here
                    _ => {}
                }
                state[cur as usize] = IN_PATH;
                path.push(cur);
                let p = parent[cur as usize];
                if p == NONE {
                    break Some(cur); // chain ends without reaching the root
                }
                cur = p;
            };
            if let Some(top) = detached_top {
                let orphan = match orphaned {
                    Some(o) => o,
                    None => {
                        let o = self.add_orphaned(parent);
                        state.push(REACHABLE); // its parent is the root
                        orphaned = Some(o);
                        o
                    }
                };
                parent[top as usize] = orphan;
            }
            for &v in &path {
                state[v as usize] = REACHABLE;
            }
        }
    }

    /// Appends the `$Orphaned` directory under the root and returns its index.
    fn add_orphaned(&mut self, parent: &mut Vec<u32>) -> u32 {
        let idx = self.len() as u32;
        self.add_node(&EntryRecord {
            key: ORPHANED_KEY,
            parent_key: self.root_key,
            name: ORPHANED_NAME.to_string(),
            size: 0,
            alloc: 0,
            mtime: 0,
            flags: NodeFlags::DIR | NodeFlags::SYNTHETIC,
        });
        parent.push(0);
        idx
    }
}

/// Builds `first_child` / `next_sibling` columns; children keep node-index (insertion) order.
fn link_children(parent: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let n = parent.len();
    let mut first_child = vec![NONE; n];
    let mut next_sibling = vec![NONE; n];
    let mut last_child = vec![NONE; n];
    for (i, &p) in parent.iter().enumerate() {
        if p == NONE {
            continue;
        }
        let p = p as usize;
        if last_child[p] == NONE {
            first_child[p] = i as u32;
        } else {
            next_sibling[last_child[p] as usize] = i as u32;
        }
        last_child[p] = i as u32;
    }
    (first_child, next_sibling)
}

/// Breadth-first node order starting at the root (node 0).
fn bfs_order(first_child: &[u32], next_sibling: &[u32]) -> Vec<u32> {
    let mut order = Vec::with_capacity(first_child.len());
    order.push(0);
    let mut head = 0;
    while head < order.len() {
        let v = order[head] as usize;
        head += 1;
        let mut c = first_child[v];
        while c != NONE {
            order.push(c);
            c = next_sibling[c as usize];
        }
    }
    order
}
