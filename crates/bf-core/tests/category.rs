use bf_core::{
    EntryRecord, FileCategory, FileFilter, NodeFlags, NodeId, Tree, TreeBuilder, TreemapOptions,
    category_of, extension_of,
};

fn rec(key: u64, parent: u64, name: &str, size: u64, flags: NodeFlags) -> EntryRecord {
    EntryRecord {
        key,
        parent_key: parent,
        name: name.to_string(),
        size,
        alloc: size,
        mtime: 0,
        flags,
    }
}

/// root -> { a.mp4 (100), b.MKV (50), docs/ -> { c.zip (30), d.txt (20), e (10), f.TXT (5) } }
/// Directories have size 0 so subtree sizes are the sum of file sizes.
fn sample() -> Tree {
    let none = NodeFlags::empty();
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(2, 1, "a.mp4", 100, none));
    b.push(rec(3, 1, "b.MKV", 50, none));
    b.push(rec(4, 1, "docs", 0, NodeFlags::DIR));
    b.push(rec(5, 4, "c.zip", 30, none));
    b.push(rec(6, 4, "d.txt", 20, none));
    b.push(rec(7, 4, "e", 10, none));
    b.push(rec(8, 4, "f.TXT", 5, none));
    b.finish()
}

fn find(t: &Tree, name: &str) -> NodeId {
    (0..t.len() as u32)
        .map(NodeId)
        .find(|&id| t.name(id) == name)
        .expect("node exists")
}

#[test]
fn extension_of_handles_dotfiles_trailing_dots_and_long_extensions() {
    assert_eq!(extension_of(".gitignore"), None);
    assert_eq!(extension_of("a.tar.gz"), Some("gz"));
    assert_eq!(extension_of("A.MKV"), Some("MKV"));
    assert_eq!(extension_of("name."), None);
    assert_eq!(extension_of("noext"), None);
    assert_eq!(extension_of(""), None);
    assert_eq!(extension_of(".a.b"), Some("b"));
    assert_eq!(extension_of("x.0123456789abcdef"), Some("0123456789abcdef"));
    assert_eq!(extension_of("x.0123456789abcdefg"), None);
    assert_eq!(extension_of("\u{e9}.\u{fc}"), Some("\u{fc}"));
}

#[test]
fn category_of_maps_extensions_case_insensitively_and_honours_system_flag() {
    let none = NodeFlags::empty();
    assert_eq!(category_of("A.MKV", none), FileCategory::Video);
    assert_eq!(category_of("x.Iso", none), FileCategory::DiskImage);
    assert_eq!(category_of("main.rs", none), FileCategory::CodeBuild);
    assert_eq!(category_of("pic.JPG", none), FileCategory::Media);
    assert_eq!(category_of("ntdll.dll", none), FileCategory::System);
    assert_eq!(category_of("a.tar.gz", none), FileCategory::Archive);
    assert_eq!(category_of("data.bin", none), FileCategory::Other);
    assert_eq!(category_of("Makefile", none), FileCategory::Other);
    assert_eq!(
        category_of("pagefile", NodeFlags::SYSTEM),
        FileCategory::System
    );
    assert_eq!(
        category_of("movie.mp4", NodeFlags::SYSTEM),
        FileCategory::System
    );
    assert_eq!(category_of("dir.mp4", NodeFlags::DIR), FileCategory::Other);
}

#[test]
fn category_u8_round_trips_and_rejects_unknown_values() {
    for c in FileCategory::ALL {
        assert_eq!(FileCategory::from_u8(c.as_u8()), Some(c));
    }
    assert_eq!(FileCategory::from_u8(7), None);
}

#[test]
fn breakdown_totals_equal_subtree_aggregate() {
    let t = sample();
    for id in [t.root(), find(&t, "docs"), find(&t, "a.mp4")] {
        let b = t.extension_breakdown(id, 10).unwrap();
        assert_eq!(b.total_bytes, t.total(id).size);
        assert_eq!(b.total_files, t.total(id).files);
        assert_eq!(b.categories.len(), 7);
        assert_eq!(
            b.categories.iter().map(|c| c.bytes).sum::<u64>(),
            b.total_bytes
        );
    }
    let b = t.extension_breakdown(t.root(), 10).unwrap();
    let cats: Vec<_> = b.categories.iter().map(|c| c.category).collect();
    assert_eq!(cats, FileCategory::ALL);
    assert_eq!(b.categories[0].bytes, 150); // .MKV folded in with .mp4
}

#[test]
fn breakdown_orders_top_extensions_and_sums_the_remainder() {
    let t = sample();
    let b = t.extension_breakdown(find(&t, "docs"), 2).unwrap();
    let exts: Vec<_> = b
        .extensions
        .iter()
        .map(|e| (e.ext.as_str(), e.bytes, e.files))
        .collect();
    // zip = 30, txt = 25 (d.txt + f.TXT, case-folded); the extensionless 10 is the remainder.
    assert_eq!(exts, [("zip", 30, 1), ("txt", 25, 2)]);
    assert_eq!(b.other_extensions.bytes, 10);
    assert_eq!(b.other_extensions.files, 1);

    // Ties on bytes are ordered by extension, and "" (no extension) is a valid key.
    let none = NodeFlags::empty();
    let mut bld = TreeBuilder::new(1, "C:");
    bld.push(rec(2, 1, "x.b", 5, none));
    bld.push(rec(3, 1, "y.a", 5, none));
    bld.push(rec(4, 1, "z", 5, none));
    let t2 = bld.finish();
    let b2 = t2.extension_breakdown(t2.root(), 3).unwrap();
    let order: Vec<_> = b2.extensions.iter().map(|e| e.ext.as_str()).collect();
    assert_eq!(order, ["", "a", "b"]);
    let none_listed = t2.extension_breakdown(t2.root(), 0).unwrap();
    assert!(none_listed.extensions.is_empty());
    assert_eq!(none_listed.other_extensions.files, 3);
}

#[test]
fn breakdown_excludes_removed_subtree_and_rejects_bad_ids() {
    let mut t = sample();
    let docs = find(&t, "docs");
    t.remove_subtree(docs).unwrap();
    let b = t.extension_breakdown(t.root(), 10).unwrap();
    assert_eq!(b.total_bytes, 150);
    assert_eq!(b.total_files, 2);
    assert!(b.extensions.iter().all(|e| e.ext != "txt"));
    assert!(t.extension_breakdown(docs, 10).is_none());
    assert!(t.extension_breakdown(NodeId(9999), 10).is_none());
}

#[test]
fn breakdown_counts_system_attribute_files_as_system() {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(2, 1, "hiberfil.sys", 40, NodeFlags::SYSTEM));
    b.push(rec(3, 1, "swap.dat", 60, NodeFlags::SYSTEM));
    let t = b.finish();
    let br = t.extension_breakdown(t.root(), 5).unwrap();
    let system = usize::from(FileCategory::System.as_u8());
    let other = usize::from(FileCategory::Other.as_u8());
    assert_eq!(br.categories[system].bytes, 100);
    assert_eq!(br.categories[other].bytes, 0);
}

#[test]
fn file_filter_matches_category_and_restricts_to_subtree() {
    let t = sample();
    let by_cat = FileFilter {
        category: Some(FileCategory::Video),
        ..Default::default()
    };
    assert_eq!(t.largest_files(&by_cat, 10).total, 2);

    let docs = find(&t, "docs");
    let scoped = FileFilter {
        under: Some(docs.0),
        ..Default::default()
    };
    let found = t.largest_files(&scoped, 10);
    let (n, ids) = (found.total, found.ids);
    assert_eq!(n, 4);
    assert!(ids.iter().all(|&i| t.path_of(i).contains("docs")));

    let both = FileFilter {
        under: Some(docs.0),
        category: Some(FileCategory::Archive),
        ..Default::default()
    };
    assert_eq!(t.largest_files(&both, 10).total, 1);

    let outside = FileFilter {
        under: Some(docs.0),
        category: Some(FileCategory::Video),
        ..Default::default()
    };
    assert_eq!(t.largest_files(&outside, 10), Default::default());
}

#[test]
fn file_filter_under_removed_or_unknown_node_is_empty() {
    let mut t = sample();
    let docs = find(&t, "docs");
    t.remove_subtree(docs).unwrap();
    for under in [docs.0, 9999] {
        let f = FileFilter {
            under: Some(under),
            ..Default::default()
        };
        assert_eq!(t.largest_files(&f, 10), Default::default());
    }
}

#[test]
fn treemap_rects_carry_category_for_files_only() {
    let t = sample();
    let opts = TreemapOptions {
        width: 400.0,
        height: 300.0,
        min_px: 0.0,
        ..Default::default()
    };
    let rects = t.treemap(t.root(), &opts);
    assert!(rects.len() >= 5);
    for r in &rects {
        if r.is_dir {
            assert_eq!(r.category, None);
        } else {
            assert_eq!(
                r.category,
                Some(category_of(t.name(NodeId(r.id)), t.flags(NodeId(r.id))))
            );
        }
    }
    let mp4 = rects
        .iter()
        .find(|r| t.name(NodeId(r.id)) == "a.mp4")
        .unwrap();
    assert_eq!(mp4.category, Some(FileCategory::Video));
}

#[test]
fn breakdown_counts_directory_own_sizes_as_folder_bytes() {
    let none = NodeFlags::empty();
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(2, 1, "docs", 4096, NodeFlags::DIR));
    b.push(rec(3, 2, "a.txt", 100, none));
    b.push(rec(4, 2, "sub", 512, NodeFlags::DIR));
    b.push(rec(5, 4, "b.mp4", 1000, none));
    let t = b.finish();
    let docs = find(&t, "docs");
    let sub = find(&t, "sub");
    for id in [t.root(), docs, sub, find(&t, "a.txt")] {
        let br = t.extension_breakdown(id, 10).unwrap();
        assert_eq!(br.total_bytes, t.total(id).size);
    }
    let br = t.extension_breakdown(docs, 10).unwrap();
    assert_eq!(br.folder_bytes, 4096 + 512);
    assert_eq!(br.total_bytes, 4096 + 512 + 1100);
    assert_eq!(t.extension_breakdown(sub, 10).unwrap().folder_bytes, 512);
    assert_eq!(
        t.extension_breakdown(find(&t, "a.txt"), 10)
            .unwrap()
            .folder_bytes,
        0
    );
}
