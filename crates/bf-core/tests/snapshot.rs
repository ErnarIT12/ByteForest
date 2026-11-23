use std::io::Write;

use bf_core::{
    EntryRecord, NodeFlags, NodeId, SnapshotError, SnapshotLimits, Tree, TreeBuilder,
    read_snapshot, read_snapshot_header, read_snapshot_with_limits, write_snapshot,
};
use flate2::Compression;
use flate2::write::DeflateEncoder;

fn rec(key: u64, parent: u64, name: &str, size: u64, dir: bool) -> EntryRecord {
    EntryRecord {
        key,
        parent_key: parent,
        name: name.to_string(),
        size,
        alloc: size + 100,
        mtime: -5 + key as i64 * 1_000_000_007,
        flags: if dir {
            NodeFlags::DIR
        } else {
            NodeFlags::HIDDEN
        },
    }
}

fn sample() -> Tree {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(10, 1, "Users", 4096, true));
    b.push(rec(11, 1, "Windows \u{00e9}\u{4e2d}", 0, true));
    b.push(rec(20, 10, "alice", 0, true));
    b.push(rec(21, 10, "notes.txt", 70, false));
    b.push(rec(30, 20, "a.mp4", u64::MAX / 4, false));
    b.push(rec(32, 11, "explorer.exe", 900, false));
    b.push(rec(33, 1, "pagefile.sys", 5000, false));
    b.finish()
}

fn encode(tree: &Tree) -> Vec<u8> {
    let mut out = Vec::new();
    write_snapshot(tree, "C:\\data", 1_700_000_000_123, &mut out).unwrap();
    out
}

/// Preorder listing of (depth, name, size, alloc, mtime, flags, total size).
fn listing(t: &Tree) -> Vec<(usize, String, u64, u64, i64, u16, u64)> {
    fn walk(
        t: &Tree,
        id: NodeId,
        depth: usize,
        out: &mut Vec<(usize, String, u64, u64, i64, u16, u64)>,
    ) {
        let total = t.total(id);
        out.push((
            depth,
            t.name(id).to_string(),
            total.size,
            total.alloc,
            t.mtime(id),
            t.flags(id).bits(),
            total.files * 1000 + total.dirs,
        ));
        for c in t.children_ids(id) {
            walk(t, c, depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(t, t.root(), 0, &mut out);
    out
}

/// Builds a snapshot file from raw parts so tests can break one field at a time.
struct Raw {
    version: u16,
    label: Vec<u8>,
    node_count: u32,
    payload_len: Option<u64>,
    crc: Option<u32>,
    payload: Vec<u8>,
    trailing: Vec<u8>,
}

impl Raw {
    fn new(nodes: &[(u32, u16, &str)]) -> Self {
        let mut payload = Vec::new();
        for (parent, flags, name) in nodes {
            payload.extend_from_slice(&parent.to_le_bytes());
            payload.extend_from_slice(&flags.to_le_bytes());
            payload.extend_from_slice(&varint(name.len() as u64));
            payload.extend_from_slice(name.as_bytes());
            payload.extend_from_slice(&[1, 2, 0]); // size, alloc, mtime
        }
        Raw {
            version: 1,
            label: b"x".to_vec(),
            node_count: nodes.len() as u32,
            payload_len: None,
            crc: None,
            payload,
            trailing: Vec::new(),
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut out = b"BFSNAP".to_vec();
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&7i64.to_le_bytes());
        out.extend_from_slice(&(self.label.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.label);
        out.extend_from_slice(&self.node_count.to_le_bytes());
        let len = self.payload_len.unwrap_or(self.payload.len() as u64);
        out.extend_from_slice(&len.to_le_bytes());
        let crc = self.crc.unwrap_or_else(|| crc32fast::hash(&self.payload));
        out.extend_from_slice(&crc.to_le_bytes());
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&self.payload).unwrap();
        out.extend_from_slice(&enc.finish().unwrap());
        out.extend_from_slice(&self.trailing);
        out
    }
}

fn varint(mut v: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return out;
        }
        out.push(b | 0x80);
    }
}

const DIR: u16 = 1;
const NONE: u32 = u32::MAX;

fn good_nodes() -> Vec<(u32, u16, &'static str)> {
    vec![(NONE, DIR, "root"), (0, DIR, "d"), (1, 0, "f.txt")]
}

#[test]
fn round_trip_preserves_names_sizes_mtime_flags_and_structure() {
    let tree = sample();
    let snap = read_snapshot(encode(&tree).as_slice()).unwrap();
    assert_eq!(snap.label, "C:\\data");
    assert_eq!(snap.created_unix_ms, 1_700_000_000_123);
    assert_eq!(listing(&snap.tree), listing(&tree));
    assert_eq!(snap.tree.total(snap.tree.root()), tree.total(tree.root()));
    assert_eq!(snap.tree.len(), tree.len());
}

#[test]
fn removed_subtree_is_excluded_from_snapshot() {
    let mut tree = sample();
    let before = tree.total(tree.root()).size;
    let alice = tree
        .children_ids(NodeId(1))
        .find(|&c| tree.name(c) == "alice")
        .unwrap();
    let gone = tree.remove_subtree(alice).unwrap();
    let snap = read_snapshot(encode(&tree).as_slice()).unwrap();
    assert_eq!(snap.tree.len(), tree.len() - 2);
    assert_eq!(snap.tree.total(snap.tree.root()).size, before - gone.bytes);
    let names: Vec<_> = listing(&snap.tree).into_iter().map(|e| e.1).collect();
    assert!(!names.contains(&"alice".to_string()));
    assert!(!names.contains(&"a.mp4".to_string()));
}

#[test]
fn removed_flag_never_reaches_the_snapshot() {
    let snap = read_snapshot(encode(&sample()).as_slice()).unwrap();
    assert!(
        (0..snap.tree.len() as u32)
            .all(|i| !snap.tree.flags(NodeId(i)).contains(NodeFlags::REMOVED))
    );
}

#[test]
fn truncation_at_every_byte_offset_is_an_error() {
    let bytes = encode(&sample());
    for cut in 0..bytes.len() {
        let r = read_snapshot(&bytes[..cut]);
        assert!(
            r.is_err(),
            "truncation at {cut} of {} was accepted",
            bytes.len()
        );
    }
    assert!(read_snapshot(bytes.as_slice()).is_ok());
}

#[test]
fn flipping_any_single_byte_never_panics() {
    let bytes = encode(&sample());
    for i in 0..bytes.len() {
        let mut b = bytes.clone();
        b[i] ^= 0xa5;
        let _ = read_snapshot(b.as_slice());
    }
}

#[test]
fn valid_raw_snapshot_loads() {
    let snap = read_snapshot(Raw::new(&good_nodes()).bytes().as_slice()).unwrap();
    assert_eq!(snap.tree.len(), 3);
}

#[test]
fn bad_magic_is_reported() {
    let mut b = Raw::new(&good_nodes()).bytes();
    b[0] = b'X';
    assert_eq!(
        read_snapshot(b.as_slice()).unwrap_err(),
        SnapshotError::BadMagic
    );
}

#[test]
fn unsupported_version_is_reported() {
    let mut raw = Raw::new(&good_nodes());
    raw.version = 2;
    assert_eq!(
        read_snapshot(raw.bytes().as_slice()).unwrap_err(),
        SnapshotError::UnsupportedVersion(2)
    );
}

#[test]
fn checksum_mismatch_is_reported() {
    let mut raw = Raw::new(&good_nodes());
    raw.crc = Some(0x1234_5678);
    assert_eq!(
        read_snapshot(raw.bytes().as_slice()).unwrap_err(),
        SnapshotError::ChecksumMismatch
    );
}

#[test]
fn parent_index_not_before_child_is_rejected() {
    for bad in [1u32, 2, 99] {
        let nodes = vec![(NONE, DIR, "root"), (bad, DIR, "d")];
        let err = read_snapshot(Raw::new(&nodes).bytes().as_slice()).unwrap_err();
        assert!(matches!(err, SnapshotError::Corrupt(_)), "{err:?}");
    }
}

#[test]
fn file_used_as_parent_is_rejected() {
    let nodes = vec![(NONE, DIR, "root"), (0, 0, "file"), (1, 0, "inside")];
    let err = read_snapshot(Raw::new(&nodes).bytes().as_slice()).unwrap_err();
    assert!(matches!(err, SnapshotError::Corrupt(_)), "{err:?}");
}

#[test]
fn root_with_a_parent_or_without_dir_flag_is_rejected() {
    for nodes in [vec![(0, DIR, "root")], vec![(NONE, 0, "root")]] {
        let err = read_snapshot(Raw::new(&nodes).bytes().as_slice()).unwrap_err();
        assert!(matches!(err, SnapshotError::Corrupt(_)), "{err:?}");
    }
}

#[test]
fn over_long_name_is_rejected() {
    let long = "a".repeat(32 * 1024 + 1);
    let nodes = vec![(NONE, DIR, long.as_str())];
    let err = read_snapshot(Raw::new(&nodes).bytes().as_slice()).unwrap_err();
    assert!(matches!(err, SnapshotError::Corrupt(_)), "{err:?}");
}

#[test]
fn name_at_the_limit_is_accepted() {
    let long = "a".repeat(32 * 1024);
    let nodes = vec![(NONE, DIR, long.as_str())];
    assert!(read_snapshot(Raw::new(&nodes).bytes().as_slice()).is_ok());
}

#[test]
fn invalid_utf8_name_is_rejected() {
    let mut raw = Raw::new(&good_nodes());
    // The root name is "root"; overwrite its first byte inside the payload.
    raw.payload[7] = 0xff;
    let err = read_snapshot(raw.bytes().as_slice()).unwrap_err();
    assert!(matches!(err, SnapshotError::Corrupt(_)), "{err:?}");
}

#[test]
fn node_count_larger_than_payload_allows_is_rejected() {
    let mut raw = Raw::new(&good_nodes());
    raw.node_count = 1_000_000;
    let err = read_snapshot(raw.bytes().as_slice()).unwrap_err();
    assert!(matches!(err, SnapshotError::Corrupt(_)), "{err:?}");
}

#[test]
fn node_count_larger_than_actual_nodes_is_rejected() {
    let mut raw = Raw::new(&good_nodes());
    raw.node_count = 6; // plausible for the payload size, but only 3 nodes exist
    assert!(read_snapshot(raw.bytes().as_slice()).is_err());
}

#[test]
fn zero_node_count_is_rejected() {
    let mut raw = Raw::new(&good_nodes());
    raw.node_count = 0;
    let err = read_snapshot(raw.bytes().as_slice()).unwrap_err();
    assert!(matches!(err, SnapshotError::Corrupt(_)), "{err:?}");
}

#[test]
fn payload_length_mismatch_is_rejected() {
    let real = Raw::new(&good_nodes()).payload.len() as u64;
    for declared in [real - 1, real + 1, real + 1000] {
        let mut raw = Raw::new(&good_nodes());
        raw.payload_len = Some(declared);
        assert!(
            read_snapshot(raw.bytes().as_slice()).is_err(),
            "declared {declared}"
        );
    }
}

#[test]
fn oversized_declared_payload_is_rejected_before_decompressing() {
    let mut raw = Raw::new(&good_nodes());
    raw.payload_len = Some(u64::MAX);
    let err = read_snapshot(raw.bytes().as_slice()).unwrap_err();
    assert!(matches!(err, SnapshotError::LimitExceeded(_)), "{err:?}");
}

#[test]
fn trailing_garbage_is_rejected() {
    let mut raw = Raw::new(&good_nodes());
    raw.trailing = b"junk".to_vec();
    let err = read_snapshot(raw.bytes().as_slice()).unwrap_err();
    assert_eq!(err, SnapshotError::Corrupt("trailing data after snapshot"));
}

#[test]
fn over_long_label_is_rejected() {
    let mut raw = Raw::new(&good_nodes());
    raw.label = vec![b'x'; 4097];
    let err = read_snapshot(raw.bytes().as_slice()).unwrap_err();
    assert!(matches!(err, SnapshotError::Corrupt(_)), "{err:?}");
}

#[test]
fn varint_that_overflows_is_rejected() {
    let mut raw = Raw::new(&good_nodes());
    // Replace the root's name length (offset 6) with eleven continuation bytes.
    raw.payload.splice(6..7, [0xff; 11]);
    let err = read_snapshot(raw.bytes().as_slice()).unwrap_err();
    assert!(matches!(err, SnapshotError::Corrupt(_)), "{err:?}");
}

#[test]
fn garbage_and_empty_input_are_errors() {
    assert_eq!(
        read_snapshot(&b""[..]).unwrap_err(),
        SnapshotError::Truncated
    );
    assert!(read_snapshot(&b"hello world, not a snapshot"[..]).is_err());
}

#[test]
fn long_label_and_names_are_truncated_on_write_not_rejected() {
    let mut b = TreeBuilder::new(1, "C:");
    b.push(rec(2, 1, &"\u{00e9}".repeat(20_000), 1, false));
    let tree = b.finish();
    let mut out = Vec::new();
    write_snapshot(&tree, &"\u{4e2d}".repeat(3000), 0, &mut out).unwrap();
    let snap = read_snapshot(out.as_slice()).unwrap();
    assert!(snap.label.len() <= 4096 && !snap.label.is_empty());
    assert!(snap.tree.name(NodeId(1)).len() <= 32 * 1024);
}

/// A pseudo-random tree of `n` nodes with varied names, sizes and times (xorshift).
fn generated_tree(n: u64) -> Tree {
    let mut x = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let exts = ["txt", "dll", "png", "rs", "json", "mp4", "log", "bin"];
    let mut b = TreeBuilder::new(0, "C:");
    let mut dirs = vec![0u64];
    for key in 1..n {
        let r = next();
        let parent = dirs[(r % dirs.len() as u64) as usize];
        if r % 7 == 0 {
            b.push(rec(
                key,
                parent,
                &format!("folder_{:x}", next() % 100_000),
                0,
                true,
            ));
            dirs.push(key);
        } else {
            let name = format!(
                "file{:x}.{}",
                next() % 1_000_000,
                exts[(r >> 8) as usize % 8]
            );
            let size = next() % 5_000_000;
            let mut e = rec(key, parent, &name, size, false);
            e.mtime = 1_600_000_000_000 + (next() % 100_000_000_000) as i64;
            e.alloc = size.next_multiple_of(4096);
            b.push(e);
        }
    }
    b.finish()
}

/// A best-case-for-deflate tree: sequential names, identical sizes and times.
fn repetitive_tree(n: u64) -> Tree {
    let mut b = TreeBuilder::new(0, "C:");
    b.push(rec(1, 0, "node_modules", 0, true));
    for key in 2..n {
        let mut e = rec(key, 1, &format!("module-{key}.js"), 1024, false);
        e.mtime = 1_700_000_000_000;
        e.alloc = 4096;
        b.push(e);
    }
    b.finish()
}

fn payload_ratio(bytes: &[u8]) -> f64 {
    // payload_len sits after magic(6) version(2) reserved(2) created(8) label_len(4)
    // label node_count(4).
    let label_len = u32::from_le_bytes(bytes[18..22].try_into().unwrap()) as usize;
    let at = 22 + label_len + 4;
    let payload_len = u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
    payload_len as f64 / bytes.len() as f64
}

#[test]
fn snapshot_compression_ratio_stays_far_below_the_guard() {
    // Measured on generated 100k-node trees (see `--nocapture`): payload / file size is
    // about 1.8x for random names and sizes and about 12x for a maximally repetitive tree.
    let tree = generated_tree(100_000);
    let bytes = encode(&tree);
    let ratio = payload_ratio(&bytes);
    println!("observed payload/file ratio: {ratio:.2}");
    assert!(ratio > 1.0);
    let rep = encode(&repetitive_tree(100_000));
    let rep_ratio = payload_ratio(&rep);
    println!("observed repetitive payload/file ratio: {rep_ratio:.2}");
    assert!(rep_ratio * 4.0 <= bf_core::MAX_COMPRESSION_RATIO as f64);
    let limits = SnapshotLimits::for_compressed_len(rep.len() as u64);
    assert!(read_snapshot_with_limits(rep.as_slice(), limits).is_ok());
    let limits = SnapshotLimits::for_compressed_len(bytes.len() as u64);
    assert!(read_snapshot_with_limits(bytes.as_slice(), limits).is_ok());
}

#[test]
fn for_compressed_len_scales_with_size_and_caps_at_defaults() {
    let d = SnapshotLimits::default();
    assert_eq!(d.max_nodes, 100_000_000);
    assert_eq!(d.max_payload_len, 8 * 1024 * 1024 * 1024);
    assert_eq!(
        SnapshotLimits::for_compressed_len(1000).max_payload_len,
        1000 * bf_core::MAX_COMPRESSION_RATIO
    );
    assert_eq!(SnapshotLimits::for_compressed_len(0).max_payload_len, 0);
    assert_eq!(SnapshotLimits::for_compressed_len(u64::MAX), d);
}

#[test]
fn node_count_over_limit_is_a_limit_error() {
    let bytes = Raw::new(&good_nodes()).bytes();
    let limits = SnapshotLimits {
        max_nodes: 2,
        ..SnapshotLimits::default()
    };
    assert_eq!(
        read_snapshot_with_limits(bytes.as_slice(), limits).unwrap_err(),
        SnapshotError::LimitExceeded("too many nodes")
    );
    let ok = SnapshotLimits {
        max_nodes: 3,
        ..SnapshotLimits::default()
    };
    assert!(read_snapshot_with_limits(bytes.as_slice(), ok).is_ok());
}

#[test]
fn payload_len_over_limit_is_a_limit_error() {
    let raw = Raw::new(&good_nodes());
    let len = raw.payload.len() as u64;
    let bytes = raw.bytes();
    let limits = SnapshotLimits {
        max_payload_len: len - 1,
        ..SnapshotLimits::default()
    };
    assert_eq!(
        read_snapshot_with_limits(bytes.as_slice(), limits).unwrap_err(),
        SnapshotError::LimitExceeded("payload too large")
    );
}

#[test]
fn declared_huge_payload_is_refused_by_the_ratio_guard() {
    let mut raw = Raw::new(&good_nodes());
    raw.payload_len = Some(1 << 40);
    let bytes = raw.bytes();
    let limits = SnapshotLimits::for_compressed_len(bytes.len() as u64);
    assert!(matches!(
        read_snapshot_with_limits(bytes.as_slice(), limits).unwrap_err(),
        SnapshotError::LimitExceeded(_)
    ));
}

#[test]
fn header_reports_label_time_and_counts_without_reading_the_payload() {
    let bytes = encode(&sample());
    // Cut the file right after the header: the payload is not needed.
    let header_len = 6 + 2 + 2 + 8 + 4 + r"C:\data".len() + 4 + 8 + 4;
    let header = read_snapshot_header(&bytes[..header_len]).unwrap();
    assert_eq!(header.label, r"C:\data");
    assert_eq!(header.created_unix_ms, 1_700_000_000_123);
    assert_eq!(header.node_count, sample().len() as u32);
    assert!(header.payload_len > 0);
    let full = read_snapshot(bytes.as_slice()).unwrap();
    assert_eq!(full.label, header.label);
    assert_eq!(full.created_unix_ms, header.created_unix_ms);
}

#[test]
fn header_with_bad_magic_or_version_is_reported() {
    let mut b = Raw::new(&good_nodes()).bytes();
    b[0] = b'X';
    assert_eq!(
        read_snapshot_header(b.as_slice()).unwrap_err(),
        SnapshotError::BadMagic
    );
    let mut raw = Raw::new(&good_nodes());
    raw.version = 2;
    assert_eq!(
        read_snapshot_header(raw.bytes().as_slice()).unwrap_err(),
        SnapshotError::UnsupportedVersion(2)
    );
}

#[test]
fn header_truncated_at_every_offset_is_an_error() {
    let bytes = Raw::new(&good_nodes()).bytes();
    let header_len = 6 + 2 + 2 + 8 + 4 + 1 + 4 + 8 + 4;
    for cut in 0..header_len {
        let err = read_snapshot_header(&bytes[..cut]).unwrap_err();
        assert_eq!(err, SnapshotError::Truncated, "cut at {cut}");
    }
    assert!(read_snapshot_header(&bytes[..header_len]).is_ok());
}

#[test]
fn header_with_over_long_label_is_rejected() {
    let mut raw = Raw::new(&good_nodes());
    raw.label = vec![b'x'; 4097];
    let err = read_snapshot_header(raw.bytes().as_slice()).unwrap_err();
    assert!(matches!(err, SnapshotError::Corrupt(_)), "{err:?}");
}
