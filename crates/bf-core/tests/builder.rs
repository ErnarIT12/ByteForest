use bf_core::{DirAgg, EntryRecord, NodeFlags, NodeId, Tree, TreeBuilder};

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

fn child_named(t: &Tree, parent: NodeId, name: &str) -> NodeId {
    t.children_ids(parent)
        .find(|&c| t.name(c) == name)
        .unwrap_or_else(|| panic!("no child named {name}"))
}

#[test]
fn aggregates_sizes_bottom_up() {
    let mut b = TreeBuilder::new(5, "C:");
    b.push(rec(30, 20, "b.txt", 300, false)); // child before its parent
    b.push(rec(20, 5, "Users", 0, true));
    b.push(rec(21, 5, "a.bin", 100, false));
    let t = b.finish();
    assert_eq!(
        t.total(t.root()),
        DirAgg {
            size: 400,
            alloc: 400,
            files: 2,
            dirs: 1
        }
    );
    let users = child_named(&t, t.root(), "Users");
    assert_eq!(t.total(users).size, 300);
    assert_eq!(
        t.path_of(child_named(&t, users, "b.txt")),
        r"C:\Users\b.txt"
    );
}

#[test]
fn orphans_go_to_orphaned_folder() {
    let mut b = TreeBuilder::new(5, "C:");
    b.push(rec(21, 5, "a.bin", 100, false));
    b.push(rec(50, 999, "lost.dat", 50, false)); // parent 999 does not exist
    let t = b.finish();
    let orphaned = child_named(&t, t.root(), "$Orphaned");
    assert_eq!(t.flags(orphaned), NodeFlags::DIR | NodeFlags::SYNTHETIC);
    let lost = child_named(&t, orphaned, "lost.dat");
    assert_eq!(t.total(lost).size, 50);
    assert_eq!(t.total(t.root()).size, 150);
    assert_eq!(t.path_of(lost), r"C:\$Orphaned\lost.dat");
}

#[test]
fn parent_cycle_does_not_hang() {
    let mut b = TreeBuilder::new(5, "C:");
    b.push(rec(40, 41, "x", 10, true));
    b.push(rec(41, 40, "y", 20, true));
    let t = b.finish();
    assert_eq!(t.len(), 4); // root, $Orphaned, 40, 41
    let orphaned = child_named(&t, t.root(), "$Orphaned");
    // Both cycle members must be reachable under $Orphaned.
    let mut stack = vec![orphaned];
    let mut names = Vec::new();
    while let Some(n) = stack.pop() {
        names.push(t.name(n).to_string());
        stack.extend(t.children_ids(n));
    }
    names.sort();
    assert_eq!(names, ["$Orphaned", "x", "y"]);
    assert_eq!(t.total(t.root()).size, 30);
}

#[test]
fn empty_tree_has_only_root() {
    let t = TreeBuilder::new(5, "C:").finish();
    assert_eq!(t.len(), 1);
    assert_eq!(
        t.total(t.root()),
        DirAgg {
            size: 0,
            alloc: 0,
            files: 0,
            dirs: 0
        }
    );
    assert_eq!(t.parent(t.root()), None);
    assert_eq!(t.path_of(t.root()), "C:");
}

#[test]
fn file_total_is_its_own_size() {
    let mut b = TreeBuilder::new(5, "C:");
    let mut r = rec(21, 5, "a.bin", 100, false);
    r.alloc = 4096;
    b.push(r);
    let t = b.finish();
    let f = child_named(&t, t.root(), "a.bin");
    assert_eq!(
        t.total(f),
        DirAgg {
            size: 100,
            alloc: 4096,
            files: 1,
            dirs: 0
        }
    );
    assert_eq!(t.parent(f), Some(t.root()));
}

#[test]
fn children_keep_insertion_order() {
    let mut b = TreeBuilder::new(5, "C:");
    for (i, n) in ["c", "a", "b"].iter().enumerate() {
        b.push(rec(10 + i as u64, 5, n, 1, false));
    }
    let t = b.finish();
    let names: Vec<_> = t.children_ids(t.root()).map(|c| t.name(c)).collect();
    assert_eq!(names, ["c", "a", "b"]);
}

#[test]
fn duplicate_key_keeps_first_and_root_key_is_ignored() {
    let mut b = TreeBuilder::new(5, "C:");
    b.push(rec(10, 5, "first", 1, false));
    b.push(rec(10, 5, "second", 2, false));
    b.push(rec(5, 5, "rootdup", 3, true));
    assert_eq!(b.len(), 2); // root + "first"
    let t = b.finish();
    assert_eq!(t.len(), 2);
    assert_eq!(t.name(t.root()), "C:");
    assert_eq!(t.total(t.root()).size, 1);
}

#[test]
fn file_as_parent_is_treated_as_orphan() {
    let mut b = TreeBuilder::new(5, "C:");
    b.push(rec(10, 5, "f", 1, false));
    b.push(rec(11, 10, "child_of_file", 2, false));
    let t = b.finish();
    let orphaned = child_named(&t, t.root(), "$Orphaned");
    assert_eq!(
        t.name(child_named(&t, orphaned, "child_of_file")),
        "child_of_file"
    );
    assert_eq!(t.total(t.root()).size, 3);
}

#[test]
fn tree_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Tree>();
}
