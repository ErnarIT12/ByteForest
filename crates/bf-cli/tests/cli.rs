use assert_cmd::Command;
use predicates::prelude::*;

fn temp_dir() -> tempfile::TempDir {
    match std::env::var_os("BF_TEST_TMP") {
        Some(base) => tempfile::tempdir_in(base).unwrap(),
        None => tempfile::tempdir().unwrap(),
    }
}

fn bin() -> Command {
    Command::cargo_bin("byteforest").unwrap()
}

#[test]
fn scan_folder_json_has_stats_and_top() {
    let dir = temp_dir();
    std::fs::write(dir.path().join("a.bin"), vec![0u8; 1000]).unwrap();
    std::fs::write(dir.path().join("b.bin"), vec![0u8; 10]).unwrap();

    let out = bin()
        .arg("scan")
        .arg(dir.path())
        .arg("--json")
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is one JSON");
    assert_eq!(v["stats"]["files"], 2);
    assert_eq!(v["top"].as_array().unwrap().len(), 2);
    assert_eq!(v["top"][0]["name"], "a.bin");
}

#[test]
fn scan_folder_text_has_header_and_table() {
    let dir = temp_dir();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub").join("x.bin"), vec![0u8; 2048]).unwrap();

    bin()
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Walk"))
        .stdout(predicate::str::contains("sub\\"));
}

#[test]
fn bench_folder_json_has_fields() {
    let dir = temp_dir();
    std::fs::write(dir.path().join("a.bin"), vec![0u8; 100]).unwrap();
    let out = bin()
        .args(["bench", "--runs", "2", "--json"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["runs"].as_array().unwrap().len(), 2);
    assert!(v["nodes"].as_u64().unwrap() >= 2);
    assert!(v["peak_working_set"].is_u64());
    assert!(v["bytes_per_node"].is_f64());
    #[cfg(any(windows, target_os = "macos"))]
    {
        assert!(v["peak_working_set"].as_u64().unwrap() > 0);
        assert!(v["bytes_per_node"].as_f64().unwrap() > 0.0);
    }
}

#[test]
fn drives_json_is_array() {
    let out = bin().args(["drives", "--json"]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v.is_array());
}

#[test]
fn bad_path_exits_1() {
    bin()
        .args(["scan", r"E:\definitely\does\not\exist"])
        .assert()
        .code(1)
        .stderr(predicate::str::starts_with("error:"));
}

#[test]
fn no_args_exits_2() {
    bin().assert().code(2);
}

#[test]
fn scan_diff_reports_grown_folder_and_new_file_against_saved_snapshot() {
    let dir = temp_dir();
    let out_dir = temp_dir();
    let snap = out_dir.path().join("before.bfsnap");
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub").join("old.bin"), vec![0u8; 4096]).unwrap();

    bin()
        .arg("scan")
        .arg(dir.path())
        .arg("--save-snapshot")
        .arg(&snap)
        .assert()
        .success();
    assert!(snap.exists());
    assert!(!out_dir.path().join("before.bfsnap.tmp").exists());

    std::fs::write(dir.path().join("sub").join("new.bin"), vec![0u8; 100_000]).unwrap();
    std::fs::write(dir.path().join("sub").join("new2.bin"), vec![0u8; 50_000]).unwrap();
    let out = bin()
        .arg("scan")
        .arg(dir.path())
        .args(["--json", "--diff"])
        .arg(&snap)
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let diff = &v["diff"];
    assert_eq!(diff["total_delta"], 150_000);
    assert_eq!(diff["base_label"], dir.path().to_str().unwrap());
    assert!(diff["base_created_unix_ms"].as_i64().unwrap() > 0);
    let changes = diff["changes"].as_array().unwrap();
    let find = |suffix: &str| {
        changes
            .iter()
            .find(|c| c["path"].as_str().unwrap().ends_with(suffix))
            .unwrap_or_else(|| panic!("no change for {suffix}: {changes:?}"))
    };
    let folder = find("sub");
    assert_eq!(folder["status"], "grown");
    assert_eq!(folder["delta"], 150_000);
    let file = find("new.bin");
    assert_eq!(file["status"], "added");
    assert!(file["base"].is_null());
}

#[test]
fn scan_diff_single_new_file_hides_its_pass_through_folders() {
    let dir = temp_dir();
    let out_dir = temp_dir();
    let snap = out_dir.path().join("single.bfsnap");
    let deep = dir.path().join("sub").join("deep");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("old.bin"), vec![0u8; 4096]).unwrap();
    bin()
        .arg("scan")
        .arg(dir.path())
        .arg("--save-snapshot")
        .arg(&snap)
        .assert()
        .success();
    std::fs::write(deep.join("new.bin"), vec![0u8; 100_000]).unwrap();
    let out = bin()
        .arg("scan")
        .arg(dir.path())
        .args(["--json", "--diff"])
        .arg(&snap)
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let changes = v["diff"]["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert!(changes[0]["path"].as_str().unwrap().ends_with("new.bin"));
}

#[test]
fn scan_diff_text_shows_signed_change_and_new_marker() {
    let dir = temp_dir();
    let out_dir = temp_dir();
    let snap = out_dir.path().join("t.bfsnap");
    std::fs::write(dir.path().join("a.bin"), vec![0u8; 10]).unwrap();
    bin()
        .arg("scan")
        .arg(dir.path())
        .arg("--save-snapshot")
        .arg(&snap)
        .assert()
        .success();
    std::fs::write(dir.path().join("b.bin"), vec![0u8; 2048]).unwrap();
    bin()
        .arg("scan")
        .arg(dir.path())
        .arg("--diff")
        .arg(&snap)
        .assert()
        .success()
        .stdout(predicate::str::contains("net +2.0 KB"))
        .stdout(predicate::str::contains("new"));
}

#[test]
fn scan_diff_with_corrupt_snapshot_fails_with_clear_message() {
    let dir = temp_dir();
    let out_dir = temp_dir();
    let snap = out_dir.path().join("bad.bfsnap");
    std::fs::write(&snap, b"this is not a snapshot at all").unwrap();
    bin()
        .arg("scan")
        .arg(dir.path())
        .arg("--diff")
        .arg(&snap)
        .assert()
        .code(1)
        .stderr(predicate::str::starts_with("error: cannot read snapshot"))
        .stderr(predicate::str::contains("bad magic"))
        .stderr(predicate::str::contains("panicked").not());
}

#[test]
fn scan_diff_with_truncated_snapshot_fails_cleanly() {
    let dir = temp_dir();
    let out_dir = temp_dir();
    let snap = out_dir.path().join("t.bfsnap");
    std::fs::write(dir.path().join("a.bin"), vec![0u8; 10]).unwrap();
    bin()
        .arg("scan")
        .arg(dir.path())
        .arg("--save-snapshot")
        .arg(&snap)
        .assert()
        .success();
    let bytes = std::fs::read(&snap).unwrap();
    std::fs::write(&snap, &bytes[..bytes.len() - 3]).unwrap();
    bin()
        .arg("scan")
        .arg(dir.path())
        .arg("--diff")
        .arg(&snap)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("cannot read snapshot"))
        .stderr(predicate::str::contains("panicked").not());
}

#[test]
fn scan_diff_with_missing_snapshot_fails() {
    let dir = temp_dir();
    bin()
        .arg("scan")
        .arg(dir.path())
        .args(["--diff", "does-not-exist.bfsnap"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("cannot open snapshot"));
}

#[test]
fn scan_can_diff_against_old_snapshot_and_save_a_new_one() {
    let dir = temp_dir();
    let out_dir = temp_dir();
    let old = out_dir.path().join("old.bfsnap");
    let new = out_dir.path().join("new.bfsnap");
    std::fs::write(dir.path().join("a.bin"), vec![0u8; 10]).unwrap();
    bin()
        .arg("scan")
        .arg(dir.path())
        .arg("--save-snapshot")
        .arg(&old)
        .assert()
        .success();
    std::fs::write(dir.path().join("b.bin"), vec![0u8; 500]).unwrap();
    bin()
        .arg("scan")
        .arg(dir.path())
        .arg("--diff")
        .arg(&old)
        .arg("--save-snapshot")
        .arg(&new)
        .assert()
        .success();
    // The new snapshot reflects the folder as it is now: no change against it.
    let out = bin()
        .arg("scan")
        .arg(dir.path())
        .args(["--json", "--diff"])
        .arg(&new)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["diff"]["total_delta"], 0);
    assert!(v["diff"]["changes"].as_array().unwrap().is_empty());
}

fn sample_tree(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("node_modules").join("pkg")).unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("node_modules").join("pkg").join("x.js"),
        vec![0u8; 5000],
    )
    .unwrap();
    std::fs::write(dir.join("src").join("main.rs"), vec![0u8; 300]).unwrap();
    std::fs::write(dir.join("notes.tmp"), vec![0u8; 40]).unwrap();
}

#[test]
fn scan_exclude_removes_names_and_reports_them_in_json() {
    let dir = temp_dir();
    sample_tree(dir.path());
    let out = bin()
        .arg("scan")
        .arg(dir.path())
        .args(["--json", "--exclude", "node_modules", "--exclude", "*.tmp"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["stats"]["files"], 1);
    assert_eq!(v["stats"]["bytes"], 300);
    assert_eq!(v["stats"]["excluded"]["items"], 2);
    assert_eq!(v["stats"]["excluded"]["bytes"], 5040);
    let top: Vec<_> = v["top"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].clone())
        .collect();
    assert_eq!(top, vec![serde_json::json!("src")]);
}

#[test]
fn scan_exclude_absolute_path_below_target() {
    let dir = temp_dir();
    sample_tree(dir.path());
    let excluded = dir.path().join("src");
    let out = bin()
        .arg("scan")
        .arg(dir.path())
        .arg("--json")
        .arg("--exclude")
        .arg(&excluded)
        .arg("--exclude")
        .arg(dir.path().parent().unwrap().join("elsewhere"))
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["stats"]["excluded"]["items"], 1);
    assert_eq!(v["stats"]["bytes"], 5040);
}

#[test]
fn scan_export_csv_has_header_and_rows() {
    let dir = temp_dir();
    sample_tree(dir.path());
    let report = dir.path().join("out.csv");
    bin()
        .arg("scan")
        .arg(dir.path())
        .arg("--export")
        .arg(&report)
        .args(["--export-depth", "1"])
        .assert()
        .success();
    let text = std::fs::read_to_string(&report).unwrap();
    let body = text.trim_start_matches('\u{FEFF}');
    let mut lines = body.lines();
    assert!(
        lines
            .next()
            .unwrap()
            .starts_with("depth,relative_path,name,type,size_bytes")
    );
    assert!(lines.next().unwrap().contains(",folder,5340,"));
    assert!(
        body.contains("1,node_modules,node_modules,folder,5000,"),
        "{body}"
    );
    assert!(!body.contains("x.js"), "depth 1 must not reach x.js");
}

#[test]
fn scan_export_json_parses_and_honours_row_limit() {
    let dir = temp_dir();
    sample_tree(dir.path());
    let report = dir.path().join("out.json");
    bin()
        .arg("scan")
        .arg(dir.path())
        .arg("--export")
        .arg(&report)
        .args(["--export-depth", "5", "--export-rows", "2"])
        .assert()
        .success();
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(v["rows"].as_array().unwrap().len(), 2);
    assert_eq!(v["truncated"], true);
    assert_eq!(v["rows"][1]["name"], "node_modules");
}

#[test]
fn scan_export_neutralizes_formula_names() {
    let dir = temp_dir();
    std::fs::write(dir.path().join("=HYPERLINK(1)"), b"abc").unwrap();
    let report = dir.path().join("out.csv");
    bin()
        .arg("scan")
        .arg(dir.path())
        .arg("--export")
        .arg(&report)
        .assert()
        .success();
    let text = std::fs::read_to_string(&report).unwrap();
    assert!(text.contains("'=HYPERLINK(1)"), "{text}");
    assert!(!text.contains(",=HYPERLINK"), "{text}");
}

#[test]
fn scan_export_applies_exclusions() {
    let dir = temp_dir();
    sample_tree(dir.path());
    let report = dir.path().join("out.csv");
    bin()
        .arg("scan")
        .arg(dir.path())
        .args(["--exclude", "node_modules", "--export"])
        .arg(&report)
        .args(["--export-depth", "3"])
        .assert()
        .success();
    let text = std::fs::read_to_string(&report).unwrap();
    assert!(!text.contains("node_modules"));
    assert!(text.contains("main.rs"));
}

fn is_blake3_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[test]
fn dupes_json_finds_duplicates_and_reports_summary_and_hash() {
    let dir = temp_dir();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    // Group 1: two identical 2048-byte files, plus one different 2048-byte file.
    std::fs::write(dir.path().join("big_a.bin"), vec![0xAA; 2048]).unwrap();
    std::fs::write(dir.path().join("sub").join("big_b.bin"), vec![0xAA; 2048]).unwrap();
    std::fs::write(dir.path().join("unique.bin"), vec![0xCC; 2048]).unwrap();
    // Group 2: three identical 1024-byte files.
    std::fs::write(dir.path().join("small_1.bin"), vec![0xBB; 1024]).unwrap();
    std::fs::write(dir.path().join("small_2.bin"), vec![0xBB; 1024]).unwrap();
    std::fs::write(dir.path().join("sub").join("small_3.bin"), vec![0xBB; 1024]).unwrap();

    let out = bin()
        .arg("dupes")
        .arg(dir.path())
        .args(["--min-size", "1K", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is one JSON");
    assert_eq!(v["stats"]["files"], 6);
    assert_eq!(v["summary"]["groups"], 2);
    assert_eq!(v["summary"]["files"], 5);
    assert_eq!(v["summary"]["reclaimable_bytes"], 4096);
    assert_eq!(v["summary"]["skipped"], 0);
    assert_eq!(v["summary"]["min_size"], 1024);
    assert_eq!(v["summary"]["candidates_files"], 6);
    assert_eq!(v["summary"]["candidates_bytes"], 2048 * 3 + 1024 * 3);
    assert!(v["summary"]["bytes_read"].as_u64().unwrap() > 0);

    let groups = v["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 2);

    // Equal reclaimable (2048), tie-broken by larger file size first.
    assert_eq!(groups[0]["size"], 2048);
    assert_eq!(groups[0]["reclaimable"], 2048);
    assert!(is_blake3_hex(groups[0]["hash"].as_str().unwrap()));
    let files0: Vec<&str> = groups[0]["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap())
        .collect();
    assert_eq!(files0.len(), 2);
    assert!(files0.iter().any(|p| p.ends_with("big_a.bin")));
    assert!(files0.iter().any(|p| p.ends_with("big_b.bin")));
    assert!(!files0.iter().any(|p| p.ends_with("unique.bin")));

    assert_eq!(groups[1]["size"], 1024);
    assert_eq!(groups[1]["reclaimable"], 2048);
    assert!(is_blake3_hex(groups[1]["hash"].as_str().unwrap()));
    assert_eq!(groups[1]["files"].as_array().unwrap().len(), 3);
}

#[test]
fn dupes_json_respects_min_size_and_top() {
    let dir = temp_dir();
    std::fs::write(dir.path().join("large_1.bin"), vec![1u8; 4096]).unwrap();
    std::fs::write(dir.path().join("large_2.bin"), vec![1u8; 4096]).unwrap();
    std::fs::write(dir.path().join("small_1.bin"), vec![2u8; 1024]).unwrap();
    std::fs::write(dir.path().join("small_2.bin"), vec![2u8; 1024]).unwrap();

    // --min-size 2K skips the 1024-byte pair.
    let out = bin()
        .arg("dupes")
        .arg(dir.path())
        .args(["--min-size", "2K", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["summary"]["groups"], 1);
    assert_eq!(v["summary"]["reclaimable_bytes"], 4096);
    assert_eq!(v["summary"]["min_size"], 2048);
    assert_eq!(v["groups"].as_array().unwrap().len(), 1);

    // --top 1 limits returned groups, while summary still covers all confirmed groups.
    let out = bin()
        .arg("dupes")
        .arg(dir.path())
        .args(["--min-size", "1K", "--top", "1", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["summary"]["groups"], 2);
    assert_eq!(v["summary"]["files"], 4);
    assert_eq!(v["summary"]["reclaimable_bytes"], 5120);
    let groups = v["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0]["size"], 4096);
}

#[test]
fn dupes_json_respects_exclude() {
    let dir = temp_dir();
    std::fs::create_dir(dir.path().join("node_modules")).unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("node_modules").join("dup.js"),
        vec![7u8; 512],
    )
    .unwrap();
    std::fs::write(dir.path().join("src").join("dup.js"), vec![7u8; 512]).unwrap();
    std::fs::write(dir.path().join("src").join("keep_a.rs"), vec![8u8; 300]).unwrap();
    std::fs::write(dir.path().join("src").join("keep_b.rs"), vec![8u8; 300]).unwrap();

    let out = bin()
        .arg("dupes")
        .arg(dir.path())
        .args(["--min-size", "100", "--exclude", "node_modules", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["stats"]["excluded"]["items"], 1);
    assert_eq!(v["summary"]["groups"], 1);
    assert_eq!(v["summary"]["reclaimable_bytes"], 300);
    let files = v["groups"][0]["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert!(
        files
            .iter()
            .all(|f| !f.as_str().unwrap().contains("node_modules"))
    );
}

#[test]
fn dupes_json_respects_include_hidden() {
    let dir = temp_dir();
    std::fs::write(dir.path().join(".dot_a"), vec![3u8; 500]).unwrap();
    std::fs::write(dir.path().join(".dot_b"), vec![3u8; 500]).unwrap();
    std::fs::create_dir(dir.path().join(".hidden_dir")).unwrap();
    std::fs::write(dir.path().join(".hidden_dir").join("a.bin"), vec![4u8; 600]).unwrap();
    std::fs::write(dir.path().join(".hidden_dir").join("b.bin"), vec![4u8; 600]).unwrap();

    // Hidden dot-files and dot-folders are excluded by default.
    let out = bin()
        .arg("dupes")
        .arg(dir.path())
        .args(["--min-size", "100", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["summary"]["groups"], 0);
    assert!(v["groups"].as_array().unwrap().is_empty());

    // With --include-hidden they are checked.
    let out = bin()
        .arg("dupes")
        .arg(dir.path())
        .args(["--min-size", "100", "--include-hidden", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["summary"]["groups"], 2);
    assert_eq!(v["summary"]["files"], 4);
    assert_eq!(v["summary"]["reclaimable_bytes"], 1100);
}

#[test]
fn dupes_json_skips_hardlinks() {
    let dir = temp_dir();
    let orig = dir.path().join("orig.bin");
    let link = dir.path().join("link.bin");
    std::fs::write(&orig, vec![9u8; 2048]).unwrap();
    std::fs::hard_link(&orig, &link).unwrap();

    let out = bin()
        .arg("dupes")
        .arg(dir.path())
        .args(["--min-size", "100", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["summary"]["groups"], 0);
    assert!(v["groups"].as_array().unwrap().is_empty());
}

#[test]
fn dupes_text_prints_summary_and_group_paths() {
    let dir = temp_dir();
    let sub = dir.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("copy1.bin"), vec![5u8; 2048]).unwrap();
    std::fs::write(dir.path().join("copy2.bin"), vec![5u8; 2048]).unwrap();

    let expected_sub = format!("sub{}copy1.bin", std::path::MAIN_SEPARATOR);
    bin()
        .arg("dupes")
        .arg(dir.path())
        .args(["--min-size", "1K"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Walk"))
        .stdout(predicate::str::contains(
            "duplicates: 1 groups, 2 files, 2.0 KB reclaimable",
        ))
        .stdout(predicate::str::contains("2.0 KB x 2"))
        .stdout(predicate::str::contains(expected_sub))
        .stdout(predicate::str::contains("copy2.bin"));

    // Empty / no duplicates message.
    let empty_dir = temp_dir();
    std::fs::write(empty_dir.path().join("only.bin"), vec![6u8; 2048]).unwrap();
    bin()
        .arg("dupes")
        .arg(empty_dir.path())
        .args(["--min-size", "1K"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no duplicate files found"));
}

#[test]
fn dupes_invalid_min_size_exits_2() {
    let dir = temp_dir();
    bin()
        .arg("dupes")
        .arg(dir.path())
        .args(["--min-size", "10nope"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("invalid size unit"));
}
