use bf_core::{
    DEFAULT_DUPLICATE_MIN_SIZE, DuplicateFilter, DuplicateGroup, EntryRecord, FileCategory,
    NodeFlags, NodeId, Tree, TreeBuilder, sort_duplicate_groups,
};

const D: NodeFlags = NodeFlags::DIR;
const F: NodeFlags = NodeFlags::empty();
const MB: u64 = 1 << 20;

/// Keys double as node ids because nodes are pushed in key order after the root (0).
fn sample() -> Tree {
    let mut b = TreeBuilder::new(0, "root");
    for (key, parent_key, name, size, flags) in [
        (1, 0, "photos", 0, D),
        (2, 1, "a.jpg", 5 * MB, F),
        (3, 1, "a copy.jpg", 5 * MB, F),
        (4, 0, "backup", 0, D),
        (5, 4, "a.jpg", 5 * MB, F),
        (6, 4, "movie.mp4", 40 * MB, F),
        (7, 0, "movie.mp4", 40 * MB, F),
        (8, 0, "lonely.bin", 7 * MB, F),
        (9, 0, "small1.txt", 10, F),
        (10, 0, "small2.txt", 10, F),
        (11, 0, "empty1", 0, F),
        (12, 0, "empty2", 0, F),
        (13, 0, "link.jpg", 5 * MB, NodeFlags::HARDLINK),
        (14, 0, "sym.jpg", 5 * MB, NodeFlags::REPARSE),
        (15, 0, "ghost.jpg", 5 * MB, NodeFlags::SYNTHETIC),
        (16, 0, "hidden.jpg", 5 * MB, NodeFlags::HIDDEN),
        (17, 0, "system.jpg", 5 * MB, NodeFlags::SYSTEM),
        (18, 0, ".dot.jpg", 5 * MB, F),
        (19, 0, "$Secret", 0, D.union(NodeFlags::HIDDEN)),
        (20, 19, "inner.mp4", 40 * MB, F),
        (21, 0, "junction", 0, D.union(NodeFlags::REPARSE)),
        (22, 21, "through.mp4", 40 * MB, F),
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

fn ids(raw: &[u32]) -> Vec<NodeId> {
    raw.iter().map(|&r| NodeId(r)).collect()
}

#[test]
fn default_filter_starts_at_one_mebibyte_and_hides_hidden_files() {
    let f = DuplicateFilter::default();
    assert_eq!(f.min_size, DEFAULT_DUPLICATE_MIN_SIZE);
    assert_eq!(f.min_size, MB);
    assert!(!f.include_hidden);
    let parsed: DuplicateFilter = serde_json::from_str("{}").unwrap();
    assert_eq!(parsed, f);
}

#[test]
fn candidates_group_same_size_files_largest_size_first() {
    let t = sample();
    let c = t.duplicate_candidates(&DuplicateFilter::default());
    assert_eq!(c.groups.len(), 2);
    assert_eq!(c.groups[0].size, 40 * MB);
    assert_eq!(c.groups[0].ids, ids(&[6, 7]));
    assert_eq!(c.groups[1].size, 5 * MB);
    assert_eq!(c.groups[1].ids, ids(&[2, 3, 5]));
    assert_eq!(c.files, 5);
    assert_eq!(c.bytes, 2 * 40 * MB + 3 * 5 * MB);
}

#[test]
fn candidates_never_include_links_synthetic_or_unique_sizes() {
    let t = sample();
    let c = t.duplicate_candidates(&DuplicateFilter {
        include_hidden: true,
        ..Default::default()
    });
    let all: Vec<NodeId> = c.groups.iter().flat_map(|g| g.ids.clone()).collect();
    for never in [8, 13, 14, 15, 22] {
        assert!(
            !all.contains(&NodeId(never)),
            "node {never} must not be a candidate"
        );
    }
}

#[test]
fn hidden_system_and_dot_files_and_hidden_folders_count_only_on_request() {
    let t = sample();
    let shown = t.duplicate_candidates(&DuplicateFilter {
        include_hidden: true,
        ..Default::default()
    });
    assert_eq!(shown.groups[0].ids, ids(&[6, 7, 20]));
    assert_eq!(shown.groups[1].ids, ids(&[2, 3, 5, 16, 17, 18]));
}

#[test]
fn empty_files_never_count_even_with_a_zero_minimum() {
    let t = sample();
    let c = t.duplicate_candidates(&DuplicateFilter {
        min_size: 0,
        ..Default::default()
    });
    let sizes: Vec<u64> = c.groups.iter().map(|g| g.size).collect();
    assert_eq!(sizes, vec![40 * MB, 5 * MB, 10]);
}

#[test]
fn candidates_respect_scope_name_and_category() {
    let t = sample();
    let scoped = t.duplicate_candidates(&DuplicateFilter {
        under: Some(1),
        ..Default::default()
    });
    assert_eq!(scoped.groups.len(), 1);
    assert_eq!(scoped.groups[0].ids, ids(&[2, 3]));

    let named = t.duplicate_candidates(&DuplicateFilter {
        name: " A.JPG ".into(),
        ..Default::default()
    });
    assert_eq!(named.groups.len(), 1);
    assert_eq!(named.groups[0].ids, ids(&[2, 5]));

    let video = t.duplicate_candidates(&DuplicateFilter {
        category: Some(FileCategory::Video),
        ..Default::default()
    });
    assert_eq!(video.groups.len(), 1);
    assert_eq!(video.groups[0].ids, ids(&[6, 7]));
}

#[test]
fn unknown_or_removed_scope_matches_nothing() {
    let mut t = sample();
    assert!(
        t.duplicate_candidates(&DuplicateFilter {
            under: Some(999),
            ..Default::default()
        })
        .groups
        .is_empty()
    );
    t.remove_subtree(NodeId(1)).unwrap();
    assert!(
        t.duplicate_candidates(&DuplicateFilter {
            under: Some(1),
            ..Default::default()
        })
        .groups
        .is_empty()
    );
}

#[test]
fn removed_files_are_not_candidates() {
    let mut t = sample();
    t.remove_subtree(NodeId(3)).unwrap();
    let c = t.duplicate_candidates(&DuplicateFilter::default());
    assert_eq!(c.groups[1].ids, ids(&[2, 5]));
}

fn group(size: u64, hash: u8, raw: &[u32]) -> DuplicateGroup {
    DuplicateGroup {
        size,
        hash: [hash; 32],
        ids: ids(raw),
    }
}

#[test]
fn reclaimable_counts_every_copy_but_one() {
    assert_eq!(group(10, 0, &[1, 2, 3]).reclaimable(), 20);
    assert_eq!(group(u64::MAX, 0, &[1, 2, 3]).reclaimable(), u64::MAX);
}

#[test]
fn groups_sort_by_reclaimable_space_then_size() {
    let mut groups = vec![
        group(10, 1, &[1, 2]),
        group(5, 2, &[3, 4, 5]),
        group(100, 3, &[6, 7]),
    ];
    sort_duplicate_groups(&mut groups);
    let sizes: Vec<u64> = groups.iter().map(|g| g.size).collect();
    assert_eq!(sizes, vec![100, 10, 5]);
}

#[test]
fn pruning_drops_removed_copies_and_groups_with_one_copy_left() {
    let mut t = sample();
    let groups = vec![
        group(5 * MB, 1, &[2, 3, 5]),
        group(40 * MB, 2, &[6, 7]),
        group(1, 3, &[8, 9999]),
    ];
    t.remove_subtree(NodeId(3)).unwrap();
    t.remove_subtree(NodeId(4)).unwrap(); // takes 5 and 6 with it
    let pruned = t.prune_duplicate_groups(&groups);
    assert!(pruned.is_empty(), "{pruned:?}");

    let t = sample();
    let pruned = t.prune_duplicate_groups(&groups[..2]);
    assert_eq!(pruned, groups[..2].to_vec());
}
