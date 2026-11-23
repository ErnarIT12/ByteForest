use bf_core::{
    EntryRecord, FileFilter, NodeFlags, NodeId, RemoveError, SortKey, Tree, TreeBuilder,
    TreemapOptions,
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

/// Node ids follow push order: 1 Users, 2 Windows, 3 alice, 4 notes.txt, 5 a.mp4, 6 b.doc,
/// 7 explorer.exe, 8 pagefile.sys.
fn sample() -> Tree {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(10, 1, "Users", 0, true));
    b.push(rec(11, 1, "Windows", 0, true));
    b.push(rec(20, 10, "alice", 0, true));
    b.push(rec(21, 10, "notes.txt", 70, false));
    b.push(rec(30, 20, "a.mp4", 3000, false));
    b.push(rec(31, 20, "b.doc", 400, false));
    b.push(rec(32, 11, "explorer.exe", 900, false));
    b.push(rec(33, 1, "pagefile.sys", 5000, false));
    b.finish()
}

fn names(t: &Tree, id: u32, sort: SortKey) -> Vec<String> {
    t.children(NodeId(id), sort, 0, 100)
        .into_iter()
        .map(|v| v.name)
        .collect()
}

#[test]
fn removal_updates_ancestor_sizes_and_counts() {
    let mut t = sample();
    let before = t.total(t.root());
    let removed = t.remove_subtree(NodeId(3)).unwrap(); // alice
    assert_eq!(removed.bytes, 3400);
    assert_eq!(removed.files, 2);
    assert_eq!(removed.dirs, 1);
    let after = t.total(t.root());
    assert_eq!(after.size, before.size - 3400);
    assert_eq!(after.files, before.files - 2);
    assert_eq!(after.dirs, before.dirs - 1);
    assert_eq!(t.total(NodeId(1)).size, 70);
    assert!(t.is_removed(NodeId(3)));
    assert!(t.is_removed(NodeId(5)));
    assert!(!t.is_removed(NodeId(4)));
}

#[test]
fn removing_a_file_counts_one_file() {
    let mut t = sample();
    let removed = t.remove_subtree(NodeId(8)).unwrap();
    assert_eq!((removed.bytes, removed.files, removed.dirs), (5000, 1, 0));
}

#[test]
fn removed_child_is_no_longer_listed_or_sorted() {
    let mut t = sample();
    // Fill the caches first so the reset is exercised.
    assert_eq!(names(&t, 0, SortKey::SizeDesc)[0], "pagefile.sys");
    assert_eq!(names(&t, 0, SortKey::NameAsc).len(), 3);
    t.remove_subtree(NodeId(8)).unwrap();
    assert_eq!(names(&t, 0, SortKey::SizeDesc), ["Users", "Windows"]);
    assert_eq!(names(&t, 0, SortKey::NameAsc), ["Users", "Windows"]);
    assert_eq!(t.child_count(t.root()), 2);
    assert_eq!(t.child_position(NodeId(2), SortKey::SizeDesc), Some(1));
}

#[test]
fn removing_first_and_middle_siblings_keeps_the_list_intact() {
    let mut t = sample();
    t.remove_subtree(NodeId(1)).unwrap(); // first child
    assert_eq!(names(&t, 0, SortKey::NameAsc), ["pagefile.sys", "Windows"]);
    t.remove_subtree(NodeId(2)).unwrap();
    assert_eq!(names(&t, 0, SortKey::NameAsc), ["pagefile.sys"]);
}

#[test]
fn treemap_excludes_removed_nodes() {
    let mut t = sample();
    t.remove_subtree(NodeId(3)).unwrap();
    let opts = TreemapOptions {
        width: 800.0,
        height: 600.0,
        ..TreemapOptions::default()
    };
    let rects = t.treemap(t.root(), &opts);
    let ids: Vec<u32> = rects.iter().map(|r| r.id).collect();
    for gone in [3, 5, 6] {
        assert!(!ids.contains(&gone), "node {gone} still in treemap");
    }
    assert!(ids.contains(&8));
}

#[test]
fn largest_files_excludes_removed_descendants() {
    let mut t = sample();
    t.remove_subtree(NodeId(3)).unwrap();
    let found = t.largest_files(&FileFilter::default(), 10);
    let (count, ids) = (found.total, found.ids);
    assert_eq!(count, 3);
    assert!(!ids.contains(&NodeId(5)));
    assert!(!ids.contains(&NodeId(6)));
}

#[test]
fn invalid_removals_return_errors_and_leave_the_tree_unchanged() {
    let mut t = sample();
    assert_eq!(t.remove_subtree(NodeId(0)), Err(RemoveError::Root));
    assert_eq!(t.remove_subtree(NodeId(999)), Err(RemoveError::OutOfRange));
    t.remove_subtree(NodeId(3)).unwrap();
    let total = t.total(t.root());
    assert_eq!(
        t.remove_subtree(NodeId(3)),
        Err(RemoveError::AlreadyRemoved)
    );
    // A descendant of a removed directory is flagged too.
    assert_eq!(
        t.remove_subtree(NodeId(5)),
        Err(RemoveError::AlreadyRemoved)
    );
    assert_eq!(t.total(t.root()), total);
}

#[test]
fn clone_is_independent_of_the_original() {
    let t = sample();
    let mut copy = t.clone();
    copy.remove_subtree(NodeId(8)).unwrap();
    assert!(!t.is_removed(NodeId(8)));
    assert_eq!(t.child_count(t.root()), 3);
}

fn ids(raw: &[u32]) -> Vec<NodeId> {
    raw.iter().map(|&n| NodeId(n)).collect()
}

#[test]
fn outermost_collapses_nested_ids_into_their_ancestor() {
    let t = sample();
    // alice (3) and its files (5, 6) sit under Users (1); notes.txt (4) too.
    assert_eq!(t.outermost(&ids(&[5, 3, 6, 1, 4, 8])), ids(&[1, 8]));
    assert_eq!(t.outermost(&ids(&[5, 6, 4])), ids(&[5, 6, 4]));
}

#[test]
fn outermost_ignores_duplicates_invalid_removed_and_root_ids() {
    let mut t = sample();
    t.remove_subtree(NodeId(2)).unwrap();
    let input = ids(&[8, 8, 0, 999, u32::MAX, 2, 7, 4, 8]);
    assert_eq!(t.outermost(&input), ids(&[8, 4]));
    assert!(t.outermost(&[]).is_empty());
}

#[test]
fn outermost_keeps_the_input_order_of_survivors() {
    let t = sample();
    assert_eq!(t.outermost(&ids(&[8, 7, 6, 2, 5])), ids(&[8, 6, 2, 5]));
    assert_eq!(t.outermost(&ids(&[6, 5, 8])), ids(&[6, 5, 8]));
}
