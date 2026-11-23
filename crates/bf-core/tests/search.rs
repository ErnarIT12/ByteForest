use bf_core::{EntryRecord, FileFilter, NodeFlags, NodeId, SearchKind, Tree, TreeBuilder};

const D: NodeFlags = NodeFlags::DIR;
const F: NodeFlags = NodeFlags::empty();

/// Keys double as node ids because nodes are pushed in key order after the root (0).
fn sample() -> Tree {
    let mut b = TreeBuilder::new(0, "root");
    for (key, parent_key, name, size, flags) in [
        (1, 0, "proj", 0, D),
        (2, 1, "node_modules", 0, D),
        (3, 2, "a.js", 100, F),
        (4, 2, "node_modules", 0, D),
        (5, 4, "b.js", 50, F),
        (6, 1, "src", 0, D),
        (7, 6, "main.rs", 10, F),
        (8, 0, "other", 0, D),
        (9, 8, "Node_Modules", 0, D),
        (10, 9, "c.js", 30, F),
        (11, 0, "disk.iso", 500, F),
        (12, 8, "x.ISO", 300, F),
        (13, 8, "y.vhd", 200, F),
        (14, 8, "z.zip", 5, F),
    ] {
        b.push(EntryRecord {
            key,
            parent_key,
            name: name.to_owned(),
            size,
            alloc: size,
            mtime: 0,
            flags,
        });
    }
    b.finish()
}

fn folders(names: &[&str]) -> FileFilter {
    FileFilter {
        kind: SearchKind::Folders,
        names: names.iter().map(|n| (*n).to_owned()).collect(),
        ..Default::default()
    }
}

#[test]
fn folder_search_counts_only_outermost_matches_and_sums_subtree_totals() {
    let t = sample();
    let found = t.largest_files(&folders(&["node_modules"]), 10);
    assert_eq!(found.total, 2);
    assert_eq!(found.total_bytes, 180);
    let sizes: Vec<u64> = found.ids.iter().map(|&i| t.total(i).size).collect();
    assert_eq!(sizes, vec![150, 30]);
}

#[test]
fn folder_search_excludes_nested_match_inside_a_match() {
    let t = sample();
    let found = t.largest_files(&folders(&["node_modules"]), 10);
    assert!(!found.ids.contains(&NodeId(4)));
    assert!(found.ids.contains(&NodeId(2)));
}

#[test]
fn folder_names_match_case_insensitively_and_exactly() {
    let t = sample();
    assert_eq!(t.largest_files(&folders(&["NODE_MODULES"]), 10).total, 2);
    assert_eq!(t.largest_files(&folders(&["node_module"]), 10).total, 0);
}

#[test]
fn folder_search_ignores_removed_folders() {
    let mut t = sample();
    t.remove_subtree(NodeId(2)).unwrap();
    let found = t.largest_files(&folders(&["node_modules"]), 10);
    assert_eq!(found.total, 1);
    assert_eq!(found.total_bytes, 30);
}

#[test]
fn folder_search_respects_the_under_scope() {
    let t = sample();
    let mut f = folders(&["node_modules"]);
    f.under = Some(8);
    let found = t.largest_files(&f, 10);
    assert_eq!((found.total, found.total_bytes), (1, 30));
    f.under = Some(1);
    assert_eq!(t.largest_files(&f, 10).total_bytes, 150);
}

#[test]
fn folder_search_min_size_uses_the_total_size() {
    let t = sample();
    let mut f = folders(&["node_modules"]);
    f.min_size = 100;
    let found = t.largest_files(&f, 10);
    assert_eq!((found.total, found.total_bytes), (1, 150));
}

#[test]
fn folder_search_ignores_file_only_filters() {
    let t = sample();
    let mut f = folders(&["node_modules"]);
    f.extensions = vec!["iso".into()];
    f.extension = "zip".into();
    assert_eq!(t.largest_files(&f, 10).total, 2);
}

#[test]
fn extensions_list_matches_any_and_merges_the_legacy_field() {
    let t = sample();
    let mut f = FileFilter {
        extensions: vec![".ISO".into(), "vhd".into()],
        ..Default::default()
    };
    let found = t.largest_files(&f, 10);
    assert_eq!((found.total, found.total_bytes), (3, 1000));
    f.extension = "zip".into();
    let found = t.largest_files(&f, 10);
    assert_eq!((found.total, found.total_bytes), (4, 1005));
}

#[test]
fn file_names_match_exactly_and_ignore_ascii_case() {
    let t = sample();
    let f = FileFilter {
        names: vec!["A.JS".into()],
        ..Default::default()
    };
    let found = t.largest_files(&f, 10);
    assert_eq!(found.total, 1);
    assert_eq!(t.name(found.ids[0]), "a.js");
}

#[test]
fn total_bytes_counts_every_match_beyond_the_limit() {
    let t = sample();
    let f = FileFilter {
        extensions: vec!["iso".into(), "vhd".into()],
        ..Default::default()
    };
    let found = t.largest_files(&f, 1);
    assert_eq!(found.ids.len(), 1);
    assert_eq!((found.total, found.total_bytes), (3, 1000));
}

#[test]
fn name_substring_search_is_case_insensitive() {
    let t = sample();
    let f = FileFilter {
        name: "  DISK".into(),
        ..Default::default()
    };
    assert_eq!(t.largest_files(&f, 10).total, 1);
}
