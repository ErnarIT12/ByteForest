use bf_core::{
    EntryRecord, ExclusionSet, NodeFlags, NodeId, ReportOptions, Tree, TreeBuilder, write_csv,
};

fn rec(key: u64, parent: u64, name: &str, size: u64, dir: bool) -> EntryRecord {
    EntryRecord {
        key,
        parent_key: parent,
        name: name.to_string(),
        size,
        alloc: size + 1,
        // 2024-01-02T03:04:05Z as a FILETIME
        mtime: (1_704_164_645 + 11_644_473_600) * 10_000_000,
        flags: if dir {
            NodeFlags::DIR
        } else {
            NodeFlags::empty()
        },
    }
}

/// root -> { big -> { b1 300, deep -> d1 5 }, small 10, "=cmd" 20, `a,"b"` 1 }
///
/// Ids: 1 big, 2 b1, 3 deep, 4 d1, 5 small, 6 =cmd, 7 a,"b".
fn sample() -> Tree {
    let mut b = TreeBuilder::new(1, "root");
    b.push(rec(2, 1, "big", 0, true));
    b.push(rec(3, 2, "b1", 300, false));
    b.push(rec(4, 2, "deep", 0, true));
    b.push(rec(5, 4, "d1", 5, false));
    b.push(rec(6, 1, "small", 10, false));
    b.push(rec(7, 1, "=cmd", 20, false));
    b.push(rec(8, 1, "a,\"b\"", 1, false));
    b.finish()
}

fn opts(max_depth: u8, max_rows: usize) -> ReportOptions {
    ReportOptions {
        max_depth,
        max_rows,
        ..ReportOptions::default()
    }
}

fn names(t: &Tree, opts: ReportOptions) -> Vec<String> {
    t.report(t.root(), opts)
        .unwrap()
        .rows
        .into_iter()
        .map(|r| r.name)
        .collect()
}

fn csv(t: &Tree, opts: ReportOptions) -> String {
    let report = t.report(t.root(), opts).unwrap();
    let mut out = Vec::new();
    write_csv(&report, ',', &mut out).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn rows_follow_size_descending_depth_first_order() {
    let t = sample();
    assert_eq!(
        names(&t, opts(3, 100)),
        vec![
            "root", "big", "b1", "deep", "d1", "=cmd", "small", "a,\"b\""
        ]
    );
}

#[test]
fn depth_limit_stops_descending() {
    let t = sample();
    assert_eq!(
        names(&t, opts(1, 100)),
        vec!["root", "big", "=cmd", "small", "a,\"b\""]
    );
    assert_eq!(names(&t, opts(0, 100)), vec!["root"]);
    assert!(!t.report(t.root(), opts(1, 100)).unwrap().truncated);
}

#[test]
fn row_limit_truncates_and_flags_it() {
    let t = sample();
    let report = t.report(t.root(), opts(3, 3)).unwrap();
    assert_eq!(report.rows.len(), 3);
    assert!(report.truncated);
    assert!(!t.report(t.root(), opts(3, 8)).unwrap().truncated);
}

#[test]
fn options_are_clamped() {
    let o = opts(200, 0).clamped();
    assert_eq!((o.max_depth, o.max_rows), (16, 1));
    assert_eq!(ReportOptions::default().clamped().max_rows, 10_000);
}

#[test]
fn row_fields_are_filled() {
    let t = sample();
    let report = t.report(NodeId(1), opts(2, 100)).unwrap();
    let root = &report.rows[0];
    assert_eq!(
        (root.depth, root.relative_path.as_str(), root.is_dir),
        (0, "", true)
    );
    assert_eq!(root.size, 305);
    assert!((root.share_of_root - 1.0).abs() < 1e-9);
    let deep_file = report.rows.iter().find(|r| r.name == "d1").unwrap();
    assert_eq!(deep_file.relative_path, "deep/d1");
    assert_eq!(deep_file.depth, 2);
    assert_eq!(deep_file.modified.as_deref(), Some("2024-01-02T03:04:05Z"));
    assert!((deep_file.share_of_root - 5.0 / 305.0).abs() < 1e-9);
    assert!(t.report(NodeId(99), ReportOptions::default()).is_none());
}

#[test]
fn removed_nodes_are_not_reported() {
    let mut t = sample();
    let set = ExclusionSet::from_patterns(&["deep"], "root", false);
    t.apply_exclusions(&set, false);
    assert!(!names(&t, opts(3, 100)).contains(&"d1".to_string()));
    assert!(t.report(NodeId(3), opts(3, 100)).is_none());
}

#[test]
fn csv_has_bom_header_crlf_and_quoting() {
    let t = sample();
    let text = csv(&t, opts(2, 100));
    assert!(text.starts_with('\u{FEFF}'));
    let body = text.trim_start_matches('\u{FEFF}');
    let mut lines = body.split("\r\n");
    assert_eq!(
        lines.next().unwrap(),
        "depth,relative_path,name,type,size_bytes,allocated_bytes,files,folders,modified_utc,share_of_root"
    );
    let first = lines.next().unwrap();
    assert!(first.starts_with("0,,root,folder,336,"), "{first}");
    assert!(first.ends_with(",,1.000000"), "{first}");
    assert!(body.contains(",2024-01-02T03:04:05Z,"), "{body}");
    assert!(body.ends_with("\r\n"));
    // Comma and quote force quoting; the inner quotes are doubled.
    assert!(body.contains("\"a,\"\"b\"\"\""), "{body}");
}

#[test]
fn csv_neutralizes_formula_cells() {
    let t = sample();
    let text = csv(&t, opts(2, 100));
    assert!(text.contains(",'=cmd,"), "{text}");
    assert!(!text.contains(",=cmd,"));
    for lead in ["+", "-", "@", "\t"] {
        let mut b = TreeBuilder::new(1, "r");
        b.push(rec(2, 1, &format!("{lead}x"), 1, false));
        let t = b.finish();
        let text = csv(&t, opts(2, 100));
        assert!(text.contains(&format!("'{lead}x")), "{lead:?}: {text}");
    }
}

#[test]
fn csv_rejects_a_quote_as_delimiter() {
    let t = sample();
    let report = t.report(t.root(), ReportOptions::default()).unwrap();
    assert!(write_csv(&report, '"', &mut Vec::new()).is_err());
}

#[test]
fn json_serializes_rows_and_truncated_flag() {
    let t = sample();
    let report = t.report(t.root(), ReportOptions::default()).unwrap();
    let v = serde_json::to_value(&report).unwrap();
    assert_eq!(v["truncated"], false);
    assert_eq!(v["rows"][0]["name"], "root");
    assert_eq!(v["rows"][0]["size"], 336);
}
