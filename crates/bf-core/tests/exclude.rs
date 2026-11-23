use bf_core::{
    EntryRecord, ExclusionSet, NamePattern, NodeFlags, NodeId, SortKey, Tree, TreeBuilder,
    normalize_patterns,
};

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

/// C: -> { Users -> { alice -> { node_modules -> { x.js 100, node_modules -> y.js 50 },
/// src -> { main.rs 10, scratch.TMP 7 } } }, Windows -> { WinSxS -> big.dll 900, sys.dll 30 },
/// pagefile.sys 5000 }
///
/// Ids follow push order: 1 Users, 2 Windows, 3 alice, 4 node_modules, 5 x.js, 6 node_modules,
/// 7 y.js, 8 src, 9 main.rs, 10 scratch.TMP, 11 WinSxS, 12 big.dll, 13 sys.dll, 14 pagefile.sys.
fn sample() -> Tree {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(10, 1, "Users", 0, true));
    b.push(rec(11, 1, "Windows", 0, true));
    b.push(rec(20, 10, "alice", 0, true));
    b.push(rec(21, 20, "node_modules", 0, true));
    b.push(rec(22, 21, "x.js", 100, false));
    b.push(rec(23, 21, "node_modules", 0, true));
    b.push(rec(24, 23, "y.js", 50, false));
    b.push(rec(25, 20, "src", 0, true));
    b.push(rec(26, 25, "main.rs", 10, false));
    b.push(rec(27, 25, "scratch.TMP", 7, false));
    b.push(rec(30, 11, "WinSxS", 0, true));
    b.push(rec(31, 30, "big.dll", 900, false));
    b.push(rec(32, 11, "sys.dll", 30, false));
    b.push(rec(40, 1, "pagefile.sys", 5000, false));
    b.finish()
}

fn set(patterns: &[&str], root: &str) -> ExclusionSet {
    ExclusionSet::from_patterns(patterns, root, true)
}

fn child_names(t: &Tree, id: u32) -> Vec<String> {
    t.children(NodeId(id), SortKey::SizeDesc, 0, 100)
        .into_iter()
        .map(|v| v.name)
        .collect()
}

#[test]
fn name_glob_star_matches_extension_and_infix() {
    let p = NamePattern::new("*.tmp").unwrap();
    assert!(p.matches("a.tmp", false));
    assert!(p.matches(".tmp", false));
    assert!(!p.matches("a.tmpx", false));
    let q = NamePattern::new("a*b*c").unwrap();
    assert!(q.matches("abc", false));
    assert!(q.matches("aXXbYYc", false));
    assert!(q.matches("abbc", false));
    assert!(!q.matches("aXXbYY", false));
    assert!(NamePattern::new("*").unwrap().matches("", false));
    assert!(NamePattern::new("é*").unwrap().matches("éa", false));
}

#[test]
fn question_mark_matches_exactly_one_character() {
    let p = NamePattern::new("a?c").unwrap();
    assert!(p.matches("abc", false));
    assert!(p.matches("aéc", false));
    assert!(!p.matches("ac", false));
    assert!(!p.matches("abbc", false));
}

#[test]
fn case_flag_controls_name_comparison() {
    let p = NamePattern::new("Node_Modules").unwrap();
    assert!(!p.matches("node_modules", false));
    assert!(p.matches("node_modules", true));

    let mut t = sample();
    let none = t.apply_exclusions(
        &ExclusionSet::from_patterns(&["WINSXS"], "C:", false),
        false,
    );
    assert_eq!(none.items, 0);
    let some = t.apply_exclusions(&set(&["WINSXS"], "C:"), true);
    assert_eq!(some.items, 1);
}

#[test]
fn name_glob_removes_matching_files_and_folders() {
    let mut t = sample();
    let ex = t.apply_exclusions(&set(&["*.tmp"], "C:"), true);
    assert_eq!((ex.items, ex.bytes, ex.files, ex.dirs), (1, 7, 1, 0));
    assert_eq!(child_names(&t, 8), vec!["main.rs"]);
}

#[test]
fn path_rule_excludes_only_that_location() {
    let mut t = sample();
    let s = set(&[r"C:\Windows\WinSxS"], "C:");
    assert_eq!(s.paths, vec![vec!["Windows".to_string(), "WinSxS".into()]]);
    let ex = t.apply_exclusions(&s, true);
    assert_eq!((ex.items, ex.bytes, ex.files, ex.dirs), (1, 900, 1, 1));
    assert_eq!(child_names(&t, 2), vec!["sys.dll"]);
    assert_eq!(t.total(NodeId(2)).size, 30);
}

#[test]
fn path_rule_relative_to_folder_root_and_foreign_paths_ignored() {
    let s = set(
        &[
            r"D:\Proj\build",
            "/other/place",
            r"D:\Proj",
            r"D:\Proj\..\x",
        ],
        r"D:\Proj",
    );
    assert_eq!(s.paths, vec![vec!["build".to_string()]]);
    assert!(s.names.is_empty());
}

#[test]
fn path_rule_does_not_match_same_name_elsewhere() {
    let mut t = sample();
    let ex = t.apply_exclusions(&set(&[r"C:\WinSxS"], "C:"), true);
    assert_eq!(ex.items, 0);
}

#[test]
fn nested_match_is_counted_once() {
    let mut t = sample();
    let ex = t.apply_exclusions(&set(&["node_modules"], "C:"), true);
    assert_eq!((ex.items, ex.bytes, ex.files, ex.dirs), (1, 150, 2, 2));
    assert!(t.is_removed(NodeId(4)));
    assert!(t.is_removed(NodeId(6)));
    assert!(t.is_removed(NodeId(7)));
}

#[test]
fn totals_of_every_ancestor_are_reduced() {
    let mut t = sample();
    let before = t.total(t.root());
    let ex = t.apply_exclusions(&set(&["node_modules", "WinSxS"], "C:"), true);
    let after = t.total(t.root());
    assert_eq!(before.size - after.size, ex.bytes);
    assert_eq!(before.files - after.files, ex.files);
    assert_eq!(before.dirs - after.dirs, ex.dirs);
    assert_eq!(t.total(NodeId(3)).size, 17);
    assert_eq!(t.total(NodeId(1)).size, 17);
    assert_eq!(after.size, 17 + 30 + 5000);
}

#[test]
fn root_is_never_excluded() {
    let mut t = sample();
    let ex = t.apply_exclusions(&set(&["C:", "*"], "C:"), true);
    assert!(!t.is_removed(t.root()));
    // Everything below the root matches `*`; each top-level child is one item.
    assert_eq!(ex.items, 3);
    assert_eq!(t.total(t.root()).size, 0);
    assert!(child_names(&t, 0).is_empty());
}

#[test]
fn no_match_changes_nothing() {
    let mut t = sample();
    let before = t.total(t.root());
    let ex = t.apply_exclusions(&set(&["*.zip", r"C:\nope\nothing"], "C:"), true);
    assert_eq!(ex.items, 0);
    assert_eq!(t.total(t.root()), before);
    let empty = t.apply_exclusions(&ExclusionSet::default(), true);
    assert_eq!(empty.items, 0);
}

#[test]
fn normalize_trims_dedupes_and_caps() {
    let long = "x".repeat(261);
    let out = normalize_patterns(&["  a ", "a", "", "   ", long.as_str(), "b"]);
    assert_eq!(out, vec!["a", "b"]);
    let many: Vec<String> = (0..500).map(|i| format!("p{i}")).collect();
    assert_eq!(normalize_patterns(&many).len(), 200);
}

#[test]
fn huge_sizes_saturate_instead_of_overflowing() {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(10, 1, "skip1", 0, true));
    b.push(rec(11, 1, "skip2", 0, true));
    b.push(rec(12, 10, "a.bin", u64::MAX - 1, false));
    b.push(rec(13, 11, "b.bin", u64::MAX - 1, false));
    let mut t = b.finish();
    let ex = t.apply_exclusions(&set(&["skip*"], "C:"), true);
    assert_eq!(ex.items, 2);
    assert_eq!(ex.bytes, u64::MAX);
    assert_eq!(ex.alloc, u64::MAX);
    assert_eq!(t.total(t.root()).size, 0);
}
