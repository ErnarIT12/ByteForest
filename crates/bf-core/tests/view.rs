use bf_core::{EntryRecord, NodeFlags, NodeId, SortKey, Tree, TreeBuilder};

const ROOT_KEY: u64 = 1;

fn rec(key: u64, parent: u64, name: &str, size: u64, mtime: i64, dir: bool) -> EntryRecord {
    EntryRecord {
        key,
        parent_key: parent,
        name: name.to_string(),
        size,
        alloc: size,
        mtime,
        flags: if dir {
            NodeFlags::DIR
        } else {
            NodeFlags::empty()
        },
    }
}

/// Root with plain files `(name, size)` directly below it.
fn tree_with_files(files: &[(&str, u64)]) -> Tree {
    let mut b = TreeBuilder::new(ROOT_KEY, "C:");
    for (i, (name, size)) in files.iter().enumerate() {
        b.push(rec(100 + i as u64, ROOT_KEY, name, *size, 0, false));
    }
    b.finish()
}

fn names(t: &Tree, id: NodeId, sort: SortKey) -> Vec<String> {
    t.children(id, sort, 0, 1000)
        .into_iter()
        .map(|v| v.name)
        .collect()
}

#[test]
fn children_sorted_by_size_desc() {
    let t = tree_with_files(&[("a", 100), ("b", 300), ("c", 0)]);
    let sizes: Vec<u64> = t
        .children(t.root(), SortKey::SizeDesc, 0, 10)
        .iter()
        .map(|v| v.size)
        .collect();
    assert_eq!(sizes, [300, 100, 0]);
}

#[test]
fn children_paging() {
    let t = tree_with_files(&[("a", 100), ("b", 300), ("c", 0)]);
    let page = t.children(t.root(), SortKey::SizeDesc, 1, 1);
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].size, 100);
    assert!(t.children(t.root(), SortKey::SizeDesc, 3, 10).is_empty());
    assert!(t.children(t.root(), SortKey::SizeDesc, 99, 10).is_empty());
    // A huge offset or limit must not overflow.
    assert!(
        t.children(t.root(), SortKey::SizeDesc, usize::MAX, usize::MAX)
            .is_empty()
    );
    assert_eq!(
        t.children(t.root(), SortKey::SizeDesc, 2, usize::MAX).len(),
        1
    );
    assert!(t.children(t.root(), SortKey::SizeDesc, 0, 0).is_empty());
}

#[test]
fn limit_is_capped_at_1000() {
    let mut b = TreeBuilder::new(ROOT_KEY, "C:");
    for i in 0..1500u64 {
        b.push(rec(100 + i, ROOT_KEY, &format!("f{i}"), i, 0, false));
    }
    let t = b.finish();
    assert_eq!(t.child_count(t.root()), 1500);
    assert_eq!(t.children(t.root(), SortKey::SizeDesc, 0, 5000).len(), 1000);
    // The tail is still reachable by paging.
    assert_eq!(
        t.children(t.root(), SortKey::SizeDesc, 1000, 5000).len(),
        500
    );
}

#[test]
fn percent_of_parent() {
    let t = tree_with_files(&[("big", 300), ("small", 100)]);
    let kids = t.children(t.root(), SortKey::SizeDesc, 0, 10);
    assert_eq!(kids[0].percent_of_parent, 75.0);
    assert_eq!(kids[1].percent_of_parent, 25.0);
    // The root has no parent.
    assert_eq!(t.view(t.root()).percent_of_parent, 0.0);
}

#[test]
fn percent_of_parent_is_zero_when_parent_is_empty() {
    let t = tree_with_files(&[("z1", 0), ("z2", 0)]);
    for v in t.children(t.root(), SortKey::SizeDesc, 0, 10) {
        assert_eq!(v.percent_of_parent, 0.0);
        assert!(v.percent_of_parent.is_finite());
    }
}

#[test]
fn name_sort_case_insensitive() {
    let t = tree_with_files(&[("b", 1), ("A", 1), ("c", 1)]);
    assert_eq!(names(&t, t.root(), SortKey::NameAsc), ["A", "b", "c"]);
}

#[test]
fn modified_desc_uses_mtime() {
    let mut b = TreeBuilder::new(ROOT_KEY, "C:");
    b.push(rec(10, ROOT_KEY, "old", 1, 100, false));
    b.push(rec(11, ROOT_KEY, "new", 1, 300, false));
    b.push(rec(12, ROOT_KEY, "mid", 1, 200, false));
    let t = b.finish();
    assert_eq!(
        names(&t, t.root(), SortKey::ModifiedDesc),
        ["new", "mid", "old"]
    );
}

#[test]
fn ties_are_broken_by_name_then_id() {
    // Same size: ordered by case-insensitive name; "x" and "X" tie on name -> insertion order.
    let t = tree_with_files(&[("x", 5), ("b", 5), ("X", 5), ("a", 5)]);
    let expected = ["a", "b", "x", "X"];
    assert_eq!(names(&t, t.root(), SortKey::SizeDesc), expected);
    assert_eq!(names(&t, t.root(), SortKey::ModifiedDesc), expected);
    assert_eq!(names(&t, t.root(), SortKey::NameAsc), expected);
}

#[test]
fn repeated_calls_agree_and_each_key_has_its_own_order() {
    let t = tree_with_files(&[("b", 1), ("a", 3), ("c", 2)]);
    let by_size = names(&t, t.root(), SortKey::SizeDesc);
    let by_name = names(&t, t.root(), SortKey::NameAsc);
    assert_eq!(by_size, ["a", "c", "b"]);
    assert_eq!(by_name, ["a", "b", "c"]);
    // Second round hits the cache and must not mix the keys up.
    assert_eq!(names(&t, t.root(), SortKey::NameAsc), by_name);
    assert_eq!(names(&t, t.root(), SortKey::SizeDesc), by_size);
}

#[test]
fn view_of_directory_and_file() {
    let mut b = TreeBuilder::new(ROOT_KEY, "C:");
    b.push(rec(10, ROOT_KEY, "dir", 0, 42, true));
    b.push(rec(11, 10, "f1", 30, 0, false));
    b.push(rec(12, 10, "sub", 0, 0, true));
    b.push(rec(13, 12, "f2", 10, 0, false));
    b.push(rec(14, ROOT_KEY, "top.bin", 60, 7, false));
    let t = b.finish();

    let dir = t
        .children(t.root(), SortKey::NameAsc, 0, 10)
        .into_iter()
        .find(|v| v.name == "dir")
        .unwrap();
    assert!(dir.is_dir);
    assert!(dir.has_children);
    assert_eq!((dir.size, dir.alloc), (40, 40));
    assert_eq!((dir.files, dir.dirs), (2, 1));
    assert_eq!(dir.mtime, 42);
    assert_eq!(dir.percent_of_parent, 40.0);
    assert_eq!(t.view(NodeId(dir.id)), dir);

    let top = t
        .children_ids(t.root())
        .find(|&c| t.name(c) == "top.bin")
        .unwrap();
    let file = t.view(top);
    assert!(!file.is_dir);
    assert!(!file.has_children);
    assert_eq!((file.size, file.files, file.dirs), (60, 1, 0));
    assert_eq!(file.mtime, 7);
    assert_eq!(file.percent_of_parent, 60.0);
}

#[test]
fn empty_directory_and_files_have_no_children() {
    let mut b = TreeBuilder::new(ROOT_KEY, "C:");
    b.push(rec(10, ROOT_KEY, "empty", 0, 0, true));
    b.push(rec(11, ROOT_KEY, "file", 5, 0, false));
    let t = b.finish();
    for id in t.children_ids(t.root()) {
        assert_eq!(t.child_count(id), 0);
        assert!(t.children(id, SortKey::SizeDesc, 0, 10).is_empty());
        assert!(!t.view(id).has_children);
    }
    assert_eq!(t.child_count(t.root()), 2);
}

#[test]
fn works_from_several_threads() {
    let t = tree_with_files(&[("b", 1), ("a", 3), ("c", 2)]);
    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| {
                assert_eq!(names(&t, t.root(), SortKey::SizeDesc), ["a", "c", "b"]);
                assert_eq!(names(&t, t.root(), SortKey::NameAsc), ["a", "b", "c"]);
            });
        }
    });
}

#[test]
fn sort_key_serializes_as_variant_name() {
    assert_eq!(
        serde_json::to_string(&SortKey::SizeDesc).unwrap(),
        "\"SizeDesc\""
    );
    let k: SortKey = serde_json::from_str("\"ModifiedDesc\"").unwrap();
    assert_eq!(k, SortKey::ModifiedDesc);
    assert!(serde_json::from_str::<SortKey>("\"Nope\"").is_err());
}

#[test]
fn node_view_serializes_with_field_names() {
    let t = tree_with_files(&[("a", 4)]);
    let v = serde_json::to_value(t.view(t.root())).unwrap();
    for key in [
        "id",
        "name",
        "size",
        "alloc",
        "files",
        "dirs",
        "is_dir",
        "has_children",
        "mtime",
        "percent_of_parent",
    ] {
        assert!(v.get(key).is_some(), "missing field {key}");
    }
}

#[test]
#[should_panic(expected = "out of range")]
fn unknown_node_id_panics_with_clear_message() {
    let t = tree_with_files(&[("a", 1)]);
    t.child_count(NodeId(9999));
}
