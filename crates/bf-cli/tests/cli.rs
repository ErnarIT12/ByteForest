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
    assert!(v["peak_working_set"].as_u64().unwrap() > 0);
    assert!(v["bytes_per_node"].as_f64().unwrap() > 0.0);
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
