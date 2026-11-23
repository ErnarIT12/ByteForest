use bf_core::{DiffStatus, EntryRecord, Gone, NodeFlags, NodeId, Tree, TreeBuilder, diff};

/// `(path, size)`; paths use `/`, a trailing `/` marks a directory. Parents must come first.
fn build(items: &[(&str, u64)]) -> Tree {
    let mut b = TreeBuilder::new(0, "root");
    let mut keys: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    keys.insert(String::new(), 0);
    for (i, (path, size)) in items.iter().enumerate() {
        let is_dir = path.ends_with('/');
        let trimmed = path.trim_end_matches('/');
        let (parent, name) = match trimmed.rsplit_once('/') {
            Some((p, n)) => (p.to_string(), n),
            None => (String::new(), trimmed),
        };
        let key = i as u64 + 1;
        keys.insert(trimmed.to_string(), key);
        b.push(EntryRecord {
            key,
            parent_key: keys[&parent],
            name: name.to_string(),
            size: *size,
            alloc: *size,
            mtime: 0,
            flags: if is_dir {
                NodeFlags::DIR
            } else {
                NodeFlags::empty()
            },
        });
    }
    b.finish()
}

fn find(t: &Tree, path: &str) -> NodeId {
    let mut cur = t.root();
    for part in path.split('/').filter(|p| !p.is_empty()) {
        cur = t
            .children_ids(cur)
            .find(|&c| t.name(c) == part)
            .unwrap_or_else(|| panic!("no {part} in {path}"));
    }
    cur
}

fn base() -> Tree {
    build(&[
        ("a/", 0),
        ("a/same.bin", 100),
        ("a/grow.bin", 100),
        ("a/shrink.bin", 500),
        ("a/old.bin", 40),
        ("b/", 0),
        ("b/deep/", 0),
        ("b/deep/gone1", 10),
        ("b/deep/gone2", 20),
        ("c.txt", 7),
    ])
}

fn current() -> Tree {
    build(&[
        ("a/", 0),
        ("a/same.bin", 100),
        ("a/grow.bin", 1100),
        ("a/shrink.bin", 100),
        ("a/new.bin", 3000),
        ("b/", 0),
        ("c.txt", 7),
        ("n/", 0),
        ("n/x", 50),
    ])
}

#[test]
fn diff_classifies_added_grown_shrunk_and_unchanged() {
    let (b, c) = (base(), current());
    let d = diff(&b, &c, false);
    let st = |p: &str| d.entry(&c, find(&c, p)).status;
    assert_eq!(st("a/same.bin"), DiffStatus::Unchanged);
    assert_eq!(st("a/grow.bin"), DiffStatus::Grown);
    assert_eq!(st("a/shrink.bin"), DiffStatus::Shrunk);
    assert_eq!(st("a/new.bin"), DiffStatus::Added);
    assert_eq!(st("n"), DiffStatus::Added);
    assert_eq!(st("n/x"), DiffStatus::Added);
    assert_eq!(st("c.txt"), DiffStatus::Unchanged);

    let grow = d.entry(&c, find(&c, "a/grow.bin"));
    assert_eq!(
        (grow.base, grow.current, grow.delta),
        (Some(100), 1100, 1000)
    );
    let shrink = d.entry(&c, find(&c, "a/shrink.bin"));
    assert_eq!(shrink.delta, -400);
    let added = d.entry(&c, find(&c, "a/new.bin"));
    assert_eq!((added.base, added.delta), (None, 3000));
}

#[test]
fn directory_delta_is_net_of_all_changes_below() {
    let (b, c) = (base(), current());
    let d = diff(&b, &c, false);
    // +1000 -400 +3000 -40
    assert_eq!(d.entry(&c, find(&c, "a")).delta, 3560);
    assert_eq!(d.entry(&c, find(&c, "b")).delta, -30);
    let s = d.summary();
    assert_eq!(s.base_total, 100 + 100 + 500 + 40 + 30 + 7);
    assert_eq!(s.total_delta, 3560 - 30 + 50);
    assert_eq!(d.entry(&c, c.root()).delta, s.total_delta);
}

#[test]
fn removed_under_aggregates_nested_removals() {
    let (b, c) = (base(), current());
    let d = diff(&b, &c, false);
    // b/deep (dir + 2 files) is gone from b.
    assert_eq!(
        d.removed_under(find(&c, "b")),
        Gone {
            bytes: 30,
            count: 3
        }
    );
    // old.bin is gone from a.
    assert_eq!(
        d.removed_under(find(&c, "a")),
        Gone {
            bytes: 40,
            count: 1
        }
    );
    // The root sees both.
    assert_eq!(
        d.removed_under(c.root()),
        Gone {
            bytes: 70,
            count: 4
        }
    );
    assert_eq!(d.summary().removed, d.removed_under(c.root()));
    assert_eq!(d.removed_under(find(&c, "c.txt")), Gone::default());
}

#[test]
fn summary_counts_top_level_added_entries() {
    let (b, c) = (base(), current());
    let s = diff(&b, &c, false).summary();
    // a/new.bin plus the folder n with its file.
    assert_eq!(
        s.added,
        Gone {
            bytes: 3050,
            count: 3
        }
    );
}

#[test]
fn case_sensitive_matching_treats_different_case_as_added_and_removed() {
    let b = build(&[("Docs/", 0), ("Docs/A.TXT", 10)]);
    let c = build(&[("docs/", 0), ("docs/a.txt", 25)]);
    let d = diff(&b, &c, false);
    assert_eq!(d.entry(&c, find(&c, "docs")).status, DiffStatus::Added);
    assert_eq!(d.removed_under(c.root()).bytes, 10);
}

#[test]
fn case_insensitive_matching_pairs_names_that_differ_in_case() {
    let b = build(&[("Docs/", 0), ("Docs/A.TXT", 10)]);
    let c = build(&[("docs/", 0), ("docs/a.txt", 25)]);
    let d = diff(&b, &c, true);
    let e = d.entry(&c, find(&c, "docs/a.txt"));
    assert_eq!(
        (e.status, e.base, e.delta),
        (DiffStatus::Grown, Some(10), 15)
    );
    assert_eq!(d.removed_under(c.root()), Gone::default());
}

#[test]
fn file_and_directory_with_the_same_name_do_not_match() {
    let b = build(&[("x", 10)]);
    let c = build(&[("x/", 0), ("x/inner", 5)]);
    let d = diff(&b, &c, false);
    assert_eq!(d.entry(&c, find(&c, "x")).status, DiffStatus::Added);
    assert_eq!(d.removed_under(c.root()).bytes, 10);
}

#[test]
fn roots_match_even_when_named_differently() {
    let b = TreeBuilder::new(0, "C:").finish();
    let c = TreeBuilder::new(0, "D:").finish();
    let d = diff(&b, &c, false);
    assert_eq!(d.entry(&c, c.root()).status, DiffStatus::Unchanged);
}

#[test]
fn removed_current_nodes_are_not_compared() {
    let b = build(&[("a/", 0), ("a/f", 100)]);
    let mut c = build(&[("a/", 0), ("a/f", 100), ("a/g", 9)]);
    let g = find(&c, "a/g");
    c.remove_subtree(g).unwrap();
    let d = diff(&b, &c, false);
    assert_eq!(d.summary().total_delta, 0);
    assert_eq!(d.summary().added, Gone::default());
    assert!(d.top_changes(&c, 10).is_empty());
}

#[test]
fn unknown_node_id_gives_unchanged_entry() {
    let (b, c) = (base(), current());
    let d = diff(&b, &c, false);
    assert_eq!(d.entry(&c, NodeId(9999)).status, DiffStatus::Unchanged);
}

#[test]
fn top_changes_orders_by_growth_and_skips_root_and_nodes_inside_added_folders() {
    let (b, c) = (base(), current());
    let d = diff(&b, &c, false);
    let top = d.top_changes(&c, 10);
    let names: Vec<_> = top.iter().map(|(id, _)| c.name(*id).to_string()).collect();
    // a (+3560), new.bin (+3000), grow.bin (+1000), n (+50). Not: shrunk, unchanged, the
    // root, or n/x (inside the added folder n).
    assert_eq!(names, ["a", "new.bin", "grow.bin", "n"]);
    assert!(top.windows(2).all(|w| w[0].1.delta >= w[1].1.delta));
}

#[test]
fn top_changes_respects_limit() {
    let (b, c) = (base(), current());
    let d = diff(&b, &c, false);
    assert_eq!(d.top_changes(&c, 2).len(), 2);
    assert!(d.top_changes(&c, 0).is_empty());
}

#[test]
fn top_changes_hides_ancestors_explained_by_one_descendant() {
    let b = build(&[("a/", 0), ("a/b/", 0), ("a/b/old", 10), ("z", 1)]);
    let c = build(&[
        ("a/", 0),
        ("a/b/", 0),
        ("a/b/old", 10),
        ("a/b/big", 5000),
        ("z", 1),
    ]);
    let d = diff(&b, &c, false);
    let names: Vec<_> = d
        .top_changes(&c, 10)
        .iter()
        .map(|(id, _)| c.name(*id).to_string())
        .collect();
    assert_eq!(names, ["big"]);
}

#[test]
fn top_changes_keeps_ancestor_when_no_single_descendant_reaches_99_percent() {
    let b = build(&[("a/", 0), ("a/f", 1)]);
    // Folder grows by 1000; the larger file explains 98.9%.
    let c = build(&[("a/", 0), ("a/f", 1), ("a/g", 989), ("a/h", 11)]);
    let d = diff(&b, &c, false);
    let names: Vec<_> = d
        .top_changes(&c, 10)
        .iter()
        .map(|(id, _)| c.name(*id).to_string())
        .collect();
    assert_eq!(names, ["a", "g", "h"]);

    // At exactly 99% the folder is hidden.
    let c = build(&[("a/", 0), ("a/f", 1), ("a/g", 990), ("a/h", 10)]);
    let d = diff(&b, &c, false);
    let names: Vec<_> = d
        .top_changes(&c, 10)
        .iter()
        .map(|(id, _)| c.name(*id).to_string())
        .collect();
    assert_eq!(names, ["g", "h"]);
}

#[test]
fn top_changes_hides_ancestors_of_an_added_folder() {
    let b = build(&[("a/", 0), ("a/f", 1)]);
    let c = build(&[("a/", 0), ("a/f", 1), ("a/n/", 0), ("a/n/x", 700)]);
    let d = diff(&b, &c, false);
    let names: Vec<_> = d
        .top_changes(&c, 10)
        .iter()
        .map(|(id, _)| c.name(*id).to_string())
        .collect();
    assert_eq!(names, ["n"]);
}
