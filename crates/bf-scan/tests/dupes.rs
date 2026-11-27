use std::sync::Mutex;

use bf_core::{DuplicateFilter, Tree};
use bf_scan::{
    CancelToken, HashOptions, HashPhase, HashProgress, MAX_HASH_THREADS, MIN_HASH_THREADS,
    PARTIAL_CHUNK, ScanError, ScanRoot, Scanner, WalkScanner, default_hash_threads,
    find_duplicates, native_path_of,
};

fn temp_dir() -> tempfile::TempDir {
    match std::env::var_os("BF_TEST_TMP") {
        Some(base) => tempfile::tempdir_in(base).unwrap(),
        None => tempfile::tempdir().unwrap(),
    }
}

fn scan_dir(path: &std::path::Path) -> Tree {
    let root = ScanRoot::Folder(path.to_path_buf());
    let (tree, _) = WalkScanner
        .scan(&root, &|_| {}, &CancelToken::new())
        .unwrap();
    tree
}

#[test]
fn default_thread_count_stays_within_io_pool_bounds() {
    let n = default_hash_threads();
    assert!((MIN_HASH_THREADS..=MAX_HASH_THREADS).contains(&n));
}

#[test]
fn finds_small_and_large_duplicates_and_distinguishes_middle_differences() {
    let dir = temp_dir();
    let root = dir.path();

    // Small files (<= 2 * PARTIAL_CHUNK): confirmed in partial phase alone.
    std::fs::write(root.join("small_a.bin"), b"hello duplicate world").unwrap();
    std::fs::write(root.join("small_b.bin"), b"hello duplicate world").unwrap();
    std::fs::write(root.join("small_c.bin"), b"hello DIFFERENT world").unwrap();

    // Large files (> 2 * PARTIAL_CHUNK): same head & tail, two identical, one differs in middle.
    let big_len = (PARTIAL_CHUNK as usize) * 4;
    let mut big_a = vec![0x42u8; big_len];
    big_a[0] = 0xAA;
    big_a[big_len - 1] = 0xBB;
    let mut big_mid_diff = big_a.clone();
    big_mid_diff[big_len / 2] = 0x99;

    std::fs::write(root.join("big_1.bin"), &big_a).unwrap();
    std::fs::write(root.join("big_2.bin"), &big_a).unwrap();
    std::fs::write(root.join("big_diff_mid.bin"), &big_mid_diff).unwrap();

    // Large files with different head: rejected in partial phase without full read.
    let mut big_head_a = vec![0x11u8; big_len + 17];
    let mut big_head_b = vec![0x11u8; big_len + 17];
    big_head_a[0] = 1;
    big_head_b[0] = 2;
    std::fs::write(root.join("head_a.bin"), &big_head_a).unwrap();
    std::fs::write(root.join("head_b.bin"), &big_head_b).unwrap();

    let tree = scan_dir(root);
    let candidates = tree.duplicate_candidates(&DuplicateFilter {
        min_size: 1,
        ..Default::default()
    });

    let phases = Mutex::new(Vec::<HashPhase>::new());
    let progress = |p: HashProgress| {
        phases.lock().unwrap().push(p.phase);
    };
    let outcome = find_duplicates(
        &candidates,
        &|id| native_path_of(&tree, id),
        &HashOptions { threads: 4 },
        &progress,
        &CancelToken::new(),
    )
    .unwrap();

    assert_eq!(outcome.skipped, 0);
    assert_eq!(outcome.groups.len(), 2);

    // Largest reclaimable group first (big_1 & big_2).
    assert_eq!(outcome.groups[0].size, big_len as u64);
    let mut names0: Vec<&str> = outcome.groups[0]
        .ids
        .iter()
        .map(|&id| tree.name(id))
        .collect();
    names0.sort_unstable();
    assert_eq!(names0, ["big_1.bin", "big_2.bin"]);
    assert_eq!(outcome.groups[0].hash, *blake3::hash(&big_a).as_bytes());

    // Second group is the small files.
    let mut names1: Vec<&str> = outcome.groups[1]
        .ids
        .iter()
        .map(|&id| tree.name(id))
        .collect();
    names1.sort_unstable();
    assert_eq!(names1, ["small_a.bin", "small_b.bin"]);

    // Partial phase rejected head_a / head_b so total bytes_read is strictly less than
    // reading all 5 large files in full.
    let all_large_full = 3 * (big_len as u64) + 2 * ((big_len + 17) as u64);
    assert!(
        outcome.bytes_read < all_large_full + 4 * PARTIAL_CHUNK,
        "bytes_read={} should avoid full-reading partial mismatches",
        outcome.bytes_read
    );

    let seen = phases.into_inner().unwrap();
    assert!(seen.contains(&HashPhase::Partial));
    assert!(seen.contains(&HashPhase::Full));
}

#[test]
fn skips_hardlinks_symlinks_missing_and_resized_files() {
    let dir = temp_dir();
    let root = dir.path();

    let payload = vec![0x5Au8; 64 * 1024];
    std::fs::write(root.join("hl_a.bin"), &payload).unwrap();
    std::fs::hard_link(root.join("hl_a.bin"), root.join("hl_b.bin")).unwrap();

    std::fs::write(root.join("gone_a.bin"), &payload).unwrap();
    std::fs::write(root.join("gone_b.bin"), &payload).unwrap();

    std::fs::write(root.join("resized_a.bin"), &payload).unwrap();
    std::fs::write(root.join("resized_b.bin"), &payload).unwrap();

    let tree = scan_dir(root);

    // Mutate disk after scan: delete one file and change another's size.
    std::fs::remove_file(root.join("gone_a.bin")).unwrap();
    std::fs::write(root.join("resized_a.bin"), b"shorter").unwrap();

    let candidates = tree.duplicate_candidates(&DuplicateFilter {
        min_size: 1,
        ..Default::default()
    });
    let outcome = find_duplicates(
        &candidates,
        &|id| native_path_of(&tree, id),
        &HashOptions { threads: 2 },
        &|_| {},
        &CancelToken::new(),
    )
    .unwrap();

    // hl_a & hl_b are hardlinks (nlink == 2) -> both skipped.
    // gone_a missing, resized_a changed size -> both skipped.
    // Only gone_b and resized_b remain as identical plain files!
    assert_eq!(outcome.skipped, 4);
    assert_eq!(outcome.groups.len(), 1);
    let mut names: Vec<&str> = outcome.groups[0]
        .ids
        .iter()
        .map(|&id| tree.name(id))
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["gone_b.bin", "resized_b.bin"]);
}

#[test]
fn respects_cancel_token() {
    let dir = temp_dir();
    let root = dir.path();
    std::fs::write(root.join("a.bin"), vec![1u8; 4096]).unwrap();
    std::fs::write(root.join("b.bin"), vec![1u8; 4096]).unwrap();

    let tree = scan_dir(root);
    let candidates = tree.duplicate_candidates(&DuplicateFilter {
        min_size: 1,
        ..Default::default()
    });
    let cancel = CancelToken::new();
    cancel.cancel();

    let err = find_duplicates(
        &candidates,
        &|id| native_path_of(&tree, id),
        &HashOptions::default(),
        &|_| {},
        &cancel,
    )
    .unwrap_err();
    assert!(matches!(err, ScanError::Cancelled));
}
