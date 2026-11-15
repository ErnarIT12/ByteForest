//! Integration tests for `MftScanner` and `scan_auto`.
//!
//! `mft_and_walk_agree_on_system_drive` needs administrator rights and reads the whole system
//! drive, so it is `#[ignore]`d and also gated on `BF_ADMIN_TESTS=1`. Run it from an elevated
//! terminal. `BF_TEST_DRIVE` picks another drive letter (default: the system drive):
//!
//! ```text
//! $env:BF_ADMIN_TESTS=1; $env:BF_TEST_DRIVE='E'
//! cargo test -p bf-scan --release --test mft_vs_walk -- --include-ignored --nocapture
//! ```

use bf_core::Tree;
use bf_scan::{
    CancelToken, MftScanner, ScanMethod, ScanRoot, ScanStats, Scanner, WalkScanner, is_elevated,
    scan_auto,
};

fn system_drive() -> char {
    std::env::var("SystemDrive")
        .expect("SystemDrive is set on Windows")
        .chars()
        .next()
        .expect("SystemDrive is not empty")
        .to_ascii_uppercase()
}

/// Drive for the admin test: `BF_TEST_DRIVE` (a letter, e.g. `E`) or the system drive.
fn test_drive() -> char {
    std::env::var("BF_TEST_DRIVE")
        .ok()
        .and_then(|v| v.trim().chars().next())
        .filter(char::is_ascii_alphabetic)
        .map_or_else(system_drive, |c| c.to_ascii_uppercase())
}

fn print_stats(label: &str, tree: &Tree, stats: &ScanStats) {
    let total = tree.total(tree.root());
    println!(
        "{label}: files={} dirs={} size={} alloc={} skipped={} errors={} elapsed={} ms",
        total.files,
        total.dirs,
        total.size,
        total.alloc,
        stats.skipped_records,
        stats.errors,
        stats.elapsed_ms
    );
}

#[test]
#[ignore = "needs administrator rights; run with BF_ADMIN_TESTS=1 and --include-ignored"]
fn mft_and_walk_agree_on_system_drive() {
    if std::env::var("BF_ADMIN_TESTS").as_deref() != Ok("1") {
        eprintln!("BF_ADMIN_TESTS is not 1: skipping");
        return;
    }
    assert!(is_elevated(), "this test must run elevated");
    let drive = test_drive();
    println!("drive {drive}:");
    let root = ScanRoot::Volume(drive);
    let cancel = CancelToken::new();

    let (mft_tree, mft_stats) = MftScanner
        .scan(&root, &|_| {}, &cancel)
        .expect("MFT scan succeeds when elevated");
    let (walk_tree, walk_stats) = WalkScanner
        .scan(&root, &|_| {}, &cancel)
        .expect("walk scan succeeds");
    print_stats("mft ", &mft_tree, &mft_stats);
    print_stats("walk", &walk_tree, &walk_stats);

    assert_eq!(mft_stats.method, ScanMethod::Mft);
    assert_eq!(walk_stats.method, ScanMethod::Walk);

    let mft = mft_tree.total(mft_tree.root());
    let walk = walk_tree.total(walk_tree.root());

    // Files: within 1% (files change while the two scans run).
    let file_diff = mft.files.abs_diff(walk.files) as f64;
    assert!(
        file_diff <= walk.files as f64 * 0.01,
        "files differ by more than 1%: mft {} walk {}",
        mft.files,
        walk.files
    );

    // Sizes: the MFT also sees metadata files ($MFT, $LogFile, ...), the walker counts every
    // hard link in full (R3), hence the asymmetric bounds.
    let (mft_total, walk_total) = (mft.size as f64, walk.size as f64);
    println!("totals: mft {} walk {}", mft.size, walk.size);
    assert!(
        mft_total <= walk_total * 1.02,
        "mft total {mft_total} > walk total {walk_total} * 1.02"
    );
    assert!(
        walk_total <= mft_total * 1.25,
        "walk total {walk_total} > mft total {mft_total} * 1.25"
    );

    // Skipped records: at most 0.1% of the records seen.
    let records = mft.files + mft.dirs;
    assert!(
        mft_stats.skipped_records as f64 <= records as f64 * 0.001,
        "skipped {} of about {records} records",
        mft_stats.skipped_records
    );
}

#[test]
fn auto_falls_back_without_admin() {
    if is_elevated() {
        eprintln!("running elevated: the fallback path is not exercised");
        return;
    }
    let (_, stats) = scan_auto(
        &ScanRoot::Volume(system_drive()),
        &|_| {},
        &CancelToken::new(),
    )
    .expect("walk fallback succeeds");

    assert_eq!(stats.method, ScanMethod::Walk);
    let reason = stats.fallback_reason.expect("fallback reason is set");
    assert!(reason.contains("administrator"), "reason: {reason}");
}

#[test]
fn auto_uses_walker_for_folders_without_reason() {
    let tmp = match std::env::var_os("BF_TEST_TMP") {
        Some(base) => tempfile::tempdir_in(base).expect("create temp dir"),
        None => tempfile::tempdir().expect("create temp dir"),
    };
    std::fs::write(tmp.path().join("a.bin"), [0u8; 3]).unwrap();

    let (_, stats) = scan_auto(
        &ScanRoot::Folder(tmp.path().to_path_buf()),
        &|_| {},
        &CancelToken::new(),
    )
    .expect("folder scan succeeds");

    assert_eq!(stats.method, ScanMethod::Walk);
    assert!(stats.fallback_reason.is_none());
    assert_eq!((stats.files, stats.bytes), (1, 3));
}

#[test]
fn mft_scanner_rejects_folders() {
    let result = MftScanner.scan(
        &ScanRoot::Folder(std::env::temp_dir()),
        &|_| {},
        &CancelToken::new(),
    );
    assert!(result.is_err());
}
