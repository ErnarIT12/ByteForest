use std::f32::consts::TAU;

use bf_core::{
    EntryRecord, FileCategory, NodeFlags, NodeId, OTHER_ARC_ID, SunburstArc, SunburstOptions, Tree,
    TreeBuilder,
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

const EPS: f32 = 1e-4;

fn width(a: &SunburstArc) -> f32 {
    a.end - a.start
}

fn children_of(arcs: &[SunburstArc], parent: u32) -> Vec<&SunburstArc> {
    arcs.iter().filter(|a| a.parent == parent).collect()
}

/// Root with files 600/300/100 (ids 1, 2, 3).
fn flat_tree() -> Tree {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(10, 1, "big.mp4", 600, false));
    b.push(rec(11, 1, "mid.zip", 300, false));
    b.push(rec(12, 1, "small.txt", 100, false));
    b.finish()
}

/// root > Users(dir) > alice(dir) > a.mp4, plus notes.txt and pagefile.sys.
fn nested_tree() -> Tree {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(10, 1, "Users", 0, true));
    b.push(rec(12, 1, "pagefile.sys", 5000, false));
    b.push(rec(20, 10, "alice", 0, true));
    b.push(rec(22, 10, "notes.txt", 400, false));
    b.push(rec(30, 20, "a.mp4", 3000, false));
    b.finish()
}

#[test]
fn child_spans_sum_to_the_parent_span() {
    let tree = nested_tree();
    let arcs = tree.sunburst(tree.root(), &SunburstOptions::default());
    let ring1 = children_of(&arcs, 0);
    let sum: f32 = ring1.iter().map(|a| width(a)).sum();
    assert!((sum - TAU).abs() < EPS, "{sum}");
    let mut checked = 0;
    for parent in arcs.iter().filter(|a| a.is_dir) {
        let kids = children_of(&arcs, parent.id);
        if kids.is_empty() {
            continue;
        }
        checked += 1;
        let sum: f32 = kids.iter().map(|a| width(a)).sum();
        assert!((sum - width(parent)).abs() < EPS, "{parent:?} {sum}");
        for k in &kids {
            assert!(k.start >= parent.start - EPS && k.end <= parent.end + EPS);
        }
    }
    assert_eq!(checked, 2);
}

#[test]
fn angles_are_proportional_to_size() {
    let tree = flat_tree();
    let arcs = tree.sunburst(tree.root(), &SunburstOptions::default());
    assert_eq!(arcs.len(), 3);
    for (arc, share) in arcs.iter().zip([0.6, 0.3, 0.1]) {
        assert!((width(arc) - share * TAU).abs() < EPS, "{arc:?}");
        assert_eq!(arc.depth, 1);
        assert_eq!(arc.parent, 0);
    }
    assert!(arcs[0].start.abs() < EPS && (arcs[2].end - TAU).abs() < EPS);
}

#[test]
fn children_are_ordered_by_size_descending_then_id() {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(10, 1, "small", 10, false));
    b.push(rec(11, 1, "big", 90, false));
    b.push(rec(12, 1, "tie-a", 50, false));
    b.push(rec(13, 1, "tie-b", 50, false));
    let tree = b.finish();
    let arcs = tree.sunburst(tree.root(), &SunburstOptions::default());
    let ids: Vec<u32> = arcs.iter().map(|a| a.id).collect();
    assert_eq!(ids, vec![2, 3, 4, 1]);
    for pair in arcs.windows(2) {
        assert!((pair[0].end - pair[1].start).abs() < EPS);
    }
}

#[test]
fn narrow_children_are_merged_into_one_other_arc() {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(10, 1, "big", 9000, false));
    for i in 0..20 {
        b.push(rec(100 + i, 1, &format!("tiny{i}"), 20, false));
    }
    let tree = b.finish();
    let opts = SunburstOptions {
        min_angle: 0.05,
        ..SunburstOptions::default()
    };
    let arcs = tree.sunburst(tree.root(), &opts);
    assert_eq!(arcs.len(), 2, "{arcs:?}");
    let other = &arcs[1];
    assert_eq!((other.id, other.parent, other.depth), (OTHER_ARC_ID, 0, 1));
    assert!(!other.is_dir && other.category.is_none());
    assert!((width(other) - TAU * 400.0 / 9400.0).abs() < EPS);
    assert!((other.end - TAU).abs() < EPS);
}

#[test]
fn merged_arc_narrower_than_min_angle_is_dropped() {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(10, 1, "big", 100_000, false));
    b.push(rec(11, 1, "tiny1", 1, false));
    b.push(rec(12, 1, "tiny2", 1, false));
    let tree = b.finish();
    let arcs = tree.sunburst(tree.root(), &SunburstOptions::default());
    assert_eq!(arcs.len(), 1);
    assert_eq!(arcs[0].id, 1);
}

#[test]
fn rings_beyond_max_depth_are_not_produced() {
    let mut b = TreeBuilder::new(1, "C:");
    let mut parent = 1;
    for level in 0..10u64 {
        b.push(rec(10 + level, parent, "d", 0, true));
        parent = 10 + level;
    }
    b.push(rec(99, parent, "leaf.bin", 50, false));
    let tree = b.finish();
    let opts = SunburstOptions {
        max_depth: 3,
        ..SunburstOptions::default()
    };
    let arcs = tree.sunburst(tree.root(), &opts);
    assert_eq!(arcs.iter().map(|a| a.depth).max(), Some(3));
    assert_eq!(arcs.len(), 3);
    let deep = tree.sunburst(tree.root(), &SunburstOptions::default());
    assert_eq!(deep.iter().map(|a| a.depth).max(), Some(6));
    let zero = SunburstOptions {
        max_depth: 0,
        ..SunburstOptions::default()
    };
    assert_eq!(tree.sunburst(tree.root(), &zero).len(), 1);
}

#[test]
fn segment_cap_keeps_inner_rings_complete() {
    let mut b = TreeBuilder::new(1, "C:");
    for d in 0..5u64 {
        b.push(rec(10 + d, 1, "dir", 0, true));
        for f in 0..10u64 {
            b.push(rec(100 + d * 10 + f, 10 + d, "f.bin", 10, false));
        }
    }
    let tree = b.finish();
    let opts = SunburstOptions {
        max_segments: 12,
        ..SunburstOptions::default()
    };
    let arcs = tree.sunburst(tree.root(), &opts);
    assert_eq!(arcs.len(), 12);
    assert_eq!(arcs.iter().filter(|a| a.depth == 1).count(), 5);
    assert_eq!(arcs.iter().filter(|a| a.depth == 2).count(), 7);
    let none = SunburstOptions {
        max_segments: 0,
        ..SunburstOptions::default()
    };
    assert!(tree.sunburst(tree.root(), &none).is_empty());
}

#[test]
fn removed_subtree_is_excluded() {
    let mut tree = nested_tree();
    let before = tree.sunburst(tree.root(), &SunburstOptions::default());
    assert!(before.iter().any(|a| a.id == 1));
    tree.remove_subtree(NodeId(1)).unwrap();
    let after = tree.sunburst(tree.root(), &SunburstOptions::default());
    assert!(!after.is_empty());
    assert!(after.iter().all(|a| a.id != 1 && a.parent != 1));
    let opts = SunburstOptions::default();
    assert!(tree.sunburst(NodeId(1), &opts).is_empty());
    assert!(tree.sunburst(NodeId(9999), &opts).is_empty());
}

#[test]
fn zero_size_root_gives_no_arcs() {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(10, 1, "empty.txt", 0, false));
    b.push(rec(11, 1, "dir", 0, true));
    let tree = b.finish();
    assert!(
        tree.sunburst(tree.root(), &SunburstOptions::default())
            .is_empty()
    );
}

#[test]
fn only_files_carry_a_category() {
    let tree = nested_tree();
    let arcs = tree.sunburst(tree.root(), &SunburstOptions::default());
    assert!(arcs.iter().any(|a| a.is_dir));
    for a in &arcs {
        assert_eq!(a.category.is_some(), !a.is_dir, "{a:?}");
    }
    let video = arcs.iter().find(|a| a.id == 5).unwrap();
    assert_eq!(video.category, Some(FileCategory::Video));
}

#[test]
fn non_finite_min_angle_does_not_panic() {
    let tree = flat_tree();
    for min_angle in [f32::NAN, f32::INFINITY, -1.0] {
        let opts = SunburstOptions {
            min_angle,
            ..SunburstOptions::default()
        };
        let arcs = tree.sunburst(tree.root(), &opts);
        assert!(
            arcs.iter()
                .all(|a| a.start.is_finite() && a.end <= TAU + EPS)
        );
    }
}
