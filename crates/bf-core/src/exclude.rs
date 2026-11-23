//! Scan exclusions: names, wildcards and paths that are cut out of a finished tree.
//!
//! Matching is pure string work (no file system access), so it works on trees from any
//! scanner and stays free of OS APIs.

use std::sync::OnceLock;

use crate::flags::NodeFlags;
use crate::tree::{NONE, NodeId, Tree};

/// Most patterns a user may keep.
pub const MAX_EXCLUSION_PATTERNS: usize = 200;

/// Longest pattern, in characters.
pub const MAX_EXCLUSION_PATTERN_LEN: usize = 260;

/// A name glob: `*` matches any run of characters (also none), `?` exactly one character.
/// It is compared with a single node name, never with a path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamePattern {
    chars: Vec<char>,
}

impl NamePattern {
    /// Parses a glob; `None` for an empty pattern.
    pub fn new(pattern: &str) -> Option<Self> {
        (!pattern.is_empty()).then(|| Self {
            chars: pattern.chars().collect(),
        })
    }

    /// Whether `name` matches the whole pattern. Does not allocate.
    pub fn matches(&self, name: &str, case_insensitive: bool) -> bool {
        let pat = &self.chars;
        let (mut p, mut n) = (0usize, 0usize);
        // Pattern index after the last `*` and the name byte index it currently covers up to.
        let mut star: Option<(usize, usize)> = None;
        loop {
            if p < pat.len() && pat[p] == '*' {
                p += 1;
                star = Some((p, n));
                continue;
            }
            match name[n..].chars().next() {
                Some(c) => {
                    if p < pat.len() && (pat[p] == '?' || chars_equal(pat[p], c, case_insensitive))
                    {
                        p += 1;
                        n += c.len_utf8();
                    } else if let Some((sp, sn)) = star {
                        // Let the star swallow one more character and retry.
                        let step = name[sn..].chars().next().map_or(1, char::len_utf8);
                        star = Some((sp, sn + step));
                        p = sp;
                        n = sn + step;
                    } else {
                        return false;
                    }
                }
                None => return p == pat.len(),
            }
        }
    }
}

fn chars_equal(a: char, b: char, case_insensitive: bool) -> bool {
    a == b || (case_insensitive && a.to_lowercase().eq(b.to_lowercase()))
}

fn names_equal(a: &str, b: &str, case_insensitive: bool) -> bool {
    if case_insensitive {
        a.chars().count() == b.chars().count()
            && a.chars()
                .zip(b.chars())
                .all(|(x, y)| chars_equal(x, y, true))
    } else {
        a == b
    }
}

/// Splits on `/` and `\`, dropping empty parts.
fn components(path: &str) -> Vec<&str> {
    path.split(['/', '\\']).filter(|c| !c.is_empty()).collect()
}

/// Trims, drops empty and over-long patterns, removes duplicates and keeps at most
/// [`MAX_EXCLUSION_PATTERNS`]. The order of first occurrences is kept.
pub fn normalize_patterns<S: AsRef<str>>(patterns: &[S]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in patterns {
        let p = p.as_ref().trim();
        if p.is_empty() || p.chars().count() > MAX_EXCLUSION_PATTERN_LEN {
            continue;
        }
        if !out.iter().any(|q| q == p) {
            out.push(p.to_string());
        }
        if out.len() >= MAX_EXCLUSION_PATTERNS {
            break;
        }
    }
    out
}

/// What to cut out of a tree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExclusionSet {
    /// Globs compared with every node name.
    pub names: Vec<NamePattern>,
    /// Paths as component lists relative to the scan root (never empty lists).
    pub paths: Vec<Vec<String>>,
}

impl ExclusionSet {
    /// Whether nothing would be excluded.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty() && self.paths.is_empty()
    }

    /// Builds a set from user patterns for a scan of `root_path` (`C:`, `D:\Projects`,
    /// `/Users/me`).
    ///
    /// A pattern without `/` or `\` is a name glob. A pattern with a separator is an absolute
    /// path: it is kept when it lies strictly below `root_path` and ignored otherwise (also
    /// when it contains `.` or `..` parts). Blank patterns are ignored.
    pub fn from_patterns<S: AsRef<str>>(
        patterns: &[S],
        root_path: &str,
        case_insensitive: bool,
    ) -> Self {
        let root = components(root_path);
        let mut set = Self::default();
        for pattern in normalize_patterns(patterns) {
            if !pattern.contains(['/', '\\']) {
                set.names.extend(NamePattern::new(&pattern));
                continue;
            }
            let parts = components(&pattern);
            if parts.iter().any(|c| *c == "." || *c == "..") || parts.len() <= root.len() {
                continue;
            }
            let under_root = root
                .iter()
                .zip(&parts)
                .all(|(r, p)| names_equal(r, p, case_insensitive));
            if under_root {
                set.paths.push(
                    parts[root.len()..]
                        .iter()
                        .map(|s| (*s).to_string())
                        .collect(),
                );
            }
        }
        set
    }
}

/// What [`Tree::apply_exclusions`] cut out. Directories count themselves in `dirs`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Excluded {
    /// Outermost nodes removed (a folder with its contents is one item).
    pub items: u64,
    /// Logical bytes.
    pub bytes: u64,
    /// Allocated bytes.
    pub alloc: u64,
    /// Files removed.
    pub files: u64,
    /// Directories removed, including excluded directories themselves.
    pub dirs: u64,
}

impl Tree {
    /// Removes every node matching `set`, keeping only outermost matches (the contents of an
    /// excluded folder are not visited). The root is never excluded. Totals of all ancestors
    /// are reduced and sort caches reset.
    ///
    /// Cost: one pass over the surviving nodes; each node is compared with every name glob
    /// and with the path rules still alive on its parent chain, without allocating per node.
    /// Malformed links stop the walk instead of panicking.
    pub fn apply_exclusions(&mut self, set: &ExclusionSet, case_insensitive: bool) -> Excluded {
        let mut out = Excluded::default();
        if set.is_empty() {
            return out;
        }
        let len = self.len();
        // (directory, path rules whose first `depth` components match it, depth)
        let all_rules: Vec<u32> = (0..set.paths.len())
            .map(|i| u32::try_from(i).unwrap_or(u32::MAX))
            .collect();
        let mut stack: Vec<(u32, Vec<u32>, usize)> = vec![(0, all_rules, 0)];
        let mut visited = 0usize;
        while let Some((dir, alive, depth)) = stack.pop() {
            visited += 1;
            if visited > len {
                break;
            }
            let mut here = Excluded::default();
            let mut prev = NONE;
            let mut cur = self.first_child[dir as usize];
            let mut steps = 0usize;
            while cur != NONE && (cur as usize) < len && steps < len {
                steps += 1;
                let next = self.next_sibling[cur as usize];
                let name = self.name(NodeId(cur));
                let matched = set.names.iter().any(|p| p.matches(name, case_insensitive))
                    || alive.iter().any(|&r| {
                        let rule = &set.paths[r as usize];
                        rule.len() == depth + 1 && names_equal(&rule[depth], name, case_insensitive)
                    });
                if matched {
                    if prev == NONE {
                        self.first_child[dir as usize] = next;
                    } else {
                        self.next_sibling[prev as usize] = next;
                    }
                    self.next_sibling[cur as usize] = NONE;
                    self.exclude_subtree(cur, &mut here);
                } else {
                    if self.dir_index[cur as usize] != NONE
                        && self.first_child[cur as usize] != NONE
                    {
                        let child_alive: Vec<u32> = alive
                            .iter()
                            .copied()
                            .filter(|&r| {
                                let rule = &set.paths[r as usize];
                                rule.len() > depth + 1
                                    && names_equal(&rule[depth], name, case_insensitive)
                            })
                            .collect();
                        stack.push((cur, child_alive, depth + 1));
                    }
                    prev = cur;
                }
                cur = next;
            }
            if here.items > 0 {
                self.subtract_from_chain(dir, &here);
                out.items = out.items.saturating_add(here.items);
                out.bytes = out.bytes.saturating_add(here.bytes);
                out.alloc = out.alloc.saturating_add(here.alloc);
                out.files = out.files.saturating_add(here.files);
                out.dirs = out.dirs.saturating_add(here.dirs);
            }
        }
        out
    }

    /// Flags `root` and everything below it as removed and adds its totals to `acc`.
    fn exclude_subtree(&mut self, root: u32, acc: &mut Excluded) {
        let total = self.total(NodeId(root));
        acc.items = acc.items.saturating_add(1);
        acc.bytes = acc.bytes.saturating_add(total.size);
        acc.alloc = acc.alloc.saturating_add(total.alloc);
        acc.files = acc.files.saturating_add(total.files);
        if self.dir_index[root as usize] != NONE {
            acc.dirs = acc.dirs.saturating_add(total.dirs.saturating_add(1));
        }
        let len = self.len();
        let mut stack = vec![root];
        let mut seen = 0usize;
        while let Some(n) = stack.pop() {
            seen += 1;
            if seen > len {
                break;
            }
            self.flags[n as usize].insert(NodeFlags::REMOVED);
            let mut c = self.first_child[n as usize];
            let mut siblings = 0usize;
            while c != NONE && (c as usize) < len && siblings < len {
                stack.push(c);
                siblings += 1;
                c = self.next_sibling[c as usize];
            }
        }
    }

    /// Subtracts `removed` from `dir` and all its ancestors and resets their sort caches.
    fn subtract_from_chain(&mut self, dir: u32, removed: &Excluded) {
        let mut cur = dir;
        let mut steps = 0usize;
        while cur != NONE && (cur as usize) < self.len() && steps <= self.len() {
            steps += 1;
            let slot = self.dir_index[cur as usize];
            if slot != NONE {
                let agg = &mut self.aggs[slot as usize];
                agg.size = agg.size.saturating_sub(removed.bytes);
                agg.alloc = agg.alloc.saturating_sub(removed.alloc);
                agg.files = agg.files.saturating_sub(removed.files);
                agg.dirs = agg.dirs.saturating_sub(removed.dirs);
                self.sorted[slot as usize] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
            }
            cur = self.parent[cur as usize];
        }
    }
}
