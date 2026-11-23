use bf_core::{
    EntryRecord, NodeFlags, NodeId, Rect, Tree, TreeBuilder, TreemapOptions, header_fits,
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

fn opts(width: f32, height: f32, dir_padding: f32) -> TreemapOptions {
    TreemapOptions {
        width,
        height,
        dir_padding,
        ..TreemapOptions::default()
    }
}

fn area(r: &Rect) -> f32 {
    r.w * r.h
}

fn rect_of(rects: &[Rect], id: u32) -> &Rect {
    rects
        .iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("no rect for node {id}"))
}

/// Root (key 1) with three files of 600/300/100 bytes. Node ids are 1, 2, 3.
fn flat_tree() -> Tree {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(10, 1, "big", 600, false));
    b.push(rec(11, 1, "mid", 300, false));
    b.push(rec(12, 1, "small", 100, false));
    b.finish()
}

/// A nested tree with directories and files of varied sizes.
fn nested_tree() -> Tree {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(10, 1, "Users", 0, true));
    b.push(rec(11, 1, "Windows", 0, true));
    b.push(rec(12, 1, "pagefile.sys", 5000, false));
    b.push(rec(20, 10, "alice", 0, true));
    b.push(rec(21, 10, "bob", 0, true));
    b.push(rec(22, 10, "notes.txt", 70, false));
    b.push(rec(30, 20, "a.mp4", 3000, false));
    b.push(rec(31, 20, "b.doc", 400, false));
    b.push(rec(32, 20, "c.png", 150, false));
    b.push(rec(33, 21, "d.zip", 1200, false));
    b.push(rec(34, 21, "e.txt", 30, false));
    b.push(rec(40, 11, "System32", 0, true));
    b.push(rec(41, 11, "explorer.exe", 900, false));
    b.push(rec(42, 40, "kernel.dll", 2000, false));
    b.push(rec(43, 40, "ntdll.dll", 1500, false));
    b.push(rec(44, 40, "empty.bin", 0, false));
    b.finish()
}

#[test]
fn areas_proportional_to_size() {
    let t = flat_tree();
    let rects = t.treemap(t.root(), &opts(100.0, 100.0, 0.0));
    let areas: Vec<f32> = [1u32, 2, 3]
        .iter()
        .map(|&id| area(rect_of(&rects, id)))
        .collect();
    assert!((areas[0] - 6000.0).abs() <= 1.0, "{areas:?}");
    assert!((areas[1] - 3000.0).abs() <= 1.0, "{areas:?}");
    assert!((areas[2] - 1000.0).abs() <= 1.0, "{areas:?}");
}

#[test]
fn root_rect_comes_first_with_depth_zero() {
    let t = flat_tree();
    let rects = t.treemap(t.root(), &opts(100.0, 80.0, 0.0));
    assert_eq!(
        rects[0],
        Rect {
            id: 0,
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 80.0,
            depth: 0,
            is_dir: true,
            category: None
        }
    );
    assert!(rects[1..].iter().all(|r| r.depth == 1 && !r.is_dir));
}

#[test]
fn rects_inside_bounds() {
    let t = nested_tree();
    let (w, h) = (400.0, 300.0);
    for pad in [0.0, 1.0, 4.0] {
        for r in t.treemap(t.root(), &opts(w, h, pad)) {
            assert!(r.x >= 0.0 && r.y >= 0.0, "{r:?}");
            assert!(r.x + r.w <= w + 0.01, "{r:?}");
            assert!(r.y + r.h <= h + 0.01, "{r:?}");
        }
    }
}

#[test]
fn children_lie_inside_their_parent_rect() {
    let t = nested_tree();
    let rects = t.treemap(t.root(), &opts(400.0, 300.0, 2.0));
    for r in &rects[1..] {
        let parent = t.parent(NodeId(r.id)).unwrap();
        let p = rect_of(&rects, parent.0);
        assert!(r.x >= p.x + 2.0 - 0.01 && r.y >= p.y + 2.0 - 0.01, "{r:?}");
        assert!(r.x + r.w <= p.x + p.w - 2.0 + 0.01, "{r:?} in {p:?}");
        assert!(r.y + r.h <= p.y + p.h - 2.0 + 0.01, "{r:?} in {p:?}");
        assert_eq!(r.depth, p.depth + 1);
    }
}

#[test]
fn no_overlap_among_siblings() {
    let t = nested_tree();
    let rects = t.treemap(t.root(), &opts(400.0, 300.0, 1.0));
    for (i, a) in rects.iter().enumerate() {
        for b in &rects[i + 1..] {
            if t.parent(NodeId(a.id)) != t.parent(NodeId(b.id)) {
                continue;
            }
            let ox = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
            let oy = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
            let overlap = ox.max(0.0) * oy.max(0.0);
            assert!(overlap < 0.01, "{a:?} overlaps {b:?} by {overlap}");
        }
    }
}

#[test]
fn respects_max_rects() {
    let mut b = TreeBuilder::new(1, "C:");
    for i in 0..100_000u64 {
        b.push(rec(10 + i, 1, "f", 1, false));
    }
    let t = b.finish();
    let o = TreemapOptions {
        width: 2000.0,
        height: 2000.0,
        max_rects: 500,
        ..TreemapOptions::default()
    };
    let rects = t.treemap(t.root(), &o);
    assert!(rects.len() <= 500, "{}", rects.len());
    assert_eq!(rects.len(), 500);
}

#[test]
fn zero_size_tree_returns_only_root_rect() {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(10, 1, "empty.txt", 0, false));
    b.push(rec(11, 1, "dir", 0, true));
    let t = b.finish();
    let rects = t.treemap(t.root(), &opts(100.0, 100.0, 1.0));
    assert_eq!(rects.len(), 1);
    assert_eq!(rects[0].id, 0);
}

#[test]
fn zero_size_children_are_skipped() {
    let t = nested_tree();
    let rects = t.treemap(t.root(), &opts(400.0, 300.0, 0.0));
    // empty.bin is node 16 (insertion order after the root).
    assert_eq!(t.name(NodeId(16)), "empty.bin");
    assert!(rects.iter().all(|r| r.id != 16));
}

#[test]
fn aspect_ratio_reasonable() {
    let mut b = TreeBuilder::new(1, "C:");
    for i in 0..20u64 {
        b.push(rec(10 + i, 1, "f", 100, false));
    }
    let t = b.finish();
    let rects = t.treemap(t.root(), &opts(400.0, 300.0, 0.0));
    assert_eq!(rects.len(), 21);
    for r in &rects[1..] {
        let ratio = r.w.max(r.h) / r.w.min(r.h);
        assert!(ratio <= 3.0, "{r:?} ratio {ratio}");
    }
}

#[test]
fn small_rects_are_neither_emitted_nor_expanded() {
    let t = nested_tree();
    let o = TreemapOptions {
        width: 400.0,
        height: 300.0,
        min_px: 40.0,
        dir_padding: 0.0,
        ..TreemapOptions::default()
    };
    let rects = t.treemap(t.root(), &o);
    assert!(rects.iter().all(|r| r.w >= 40.0 && r.h >= 40.0));
    for r in &rects[1..] {
        let parent = t.parent(NodeId(r.id)).unwrap();
        assert!(rects.iter().any(|p| p.id == parent.0), "orphan {r:?}");
    }
}

#[test]
fn empty_canvas_returns_nothing() {
    let t = flat_tree();
    assert!(t.treemap(t.root(), &opts(0.0, 100.0, 0.0)).is_empty());
    assert!(t.treemap(t.root(), &opts(100.0, 0.0, 0.0)).is_empty());
    assert!(t.treemap(t.root(), &opts(f32::NAN, 100.0, 0.0)).is_empty());
    assert!(t.treemap(t.root(), &TreemapOptions::default()).is_empty());
}

#[test]
fn deep_tree_does_not_overflow_the_stack() {
    let mut b = TreeBuilder::new(1, "C:");
    let mut parent = 1;
    for i in 0..50_000u64 {
        let key = 10 + i;
        b.push(rec(key, parent, "d", 0, true));
        parent = key;
    }
    b.push(rec(10_000_000, parent, "leaf", 1, false));
    let t = b.finish();
    let o = TreemapOptions {
        width: 1000.0,
        height: 1000.0,
        dir_padding: 0.001,
        min_px: 0.0,
        ..TreemapOptions::default()
    };
    let rects = t.treemap(t.root(), &o);
    assert!(rects.len() > 1000);
    assert_eq!(rects.last().unwrap().depth, u8::MAX);
}

#[test]
fn layout_snapshot() {
    let t = nested_tree();
    let rects = t.treemap(t.root(), &opts(400.0, 300.0, 1.0));
    let lines: Vec<String> = rects
        .iter()
        .map(|r| {
            format!(
                "{:>2} {:<12} d{} {} x={:.2} y={:.2} w={:.2} h={:.2}",
                r.id,
                t.name(NodeId(r.id)),
                r.depth,
                if r.is_dir { "dir " } else { "file" },
                r.x,
                r.y,
                r.w,
                r.h
            )
        })
        .collect();
    insta::assert_yaml_snapshot!(lines);
}

#[test]
fn dir_header_reserves_a_strip_above_children_of_non_root_dirs() {
    let t = nested_tree();
    let header = 16.0;
    let o = TreemapOptions {
        width: 800.0,
        height: 600.0,
        dir_padding: 1.0,
        dir_header: header,
        ..TreemapOptions::default()
    };
    let rects = t.treemap(t.root(), &o);
    let mut checked_with_header = 0;
    for r in &rects[1..] {
        let parent = t.parent(NodeId(r.id)).unwrap();
        let p = rect_of(&rects, parent.0);
        let top = if p.depth > 0 && header_fits(p.w, p.h, header) {
            checked_with_header += 1;
            header
        } else {
            0.0
        };
        assert!(
            r.y >= p.y + 1.0 + top - 1e-3,
            "{r:?} overlaps the header of {p:?}"
        );
        assert!(r.y + r.h <= p.y + p.h - 1.0 + 1e-3, "{r:?} outside {p:?}");
    }
    assert!(checked_with_header > 0, "no directory got a header");
    // The root never gets a strip: its first child starts right below the padding.
    assert!(rects[1..].iter().any(|r| (r.y - 1.0).abs() < 1e-3));
}

#[test]
fn header_fits_needs_three_strips_each_way() {
    assert!(header_fits(48.0, 48.0, 16.0));
    assert!(!header_fits(47.9, 100.0, 16.0));
    assert!(!header_fits(100.0, 47.9, 16.0));
    assert!(!header_fits(100.0, 100.0, 0.0));
}
