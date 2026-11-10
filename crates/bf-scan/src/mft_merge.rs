//! Turns parsed MFT records into tree entries. Pure logic without I/O, so it is unit-tested
//! with hand-built records.

use bf_core::{EntryRecord, NodeFlags};
use bf_ntfs::{FileRecord, apply_fixups, parse_record};

/// Record number of the volume's root directory.
pub(crate) const ROOT_RECORD: u64 = 5;
/// Record numbers (and parent references) fit in the low 48 bits of a key.
const RECORD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
/// Extra names (hard links) put their index above the record number.
const LINK_SHIFT: u32 = 48;
/// Most names emitted per record. Keeps the link index below `0xFFFF`, so no key can equal
/// `u64::MAX` (the key `TreeBuilder` reserves for `$Orphaned`). NTFS allows 1024 links.
const MAX_NAMES: usize = 0xFFFF;

// `$STANDARD_INFORMATION` file attribute bits.
const ATTR_HIDDEN: u32 = 0x2;
const ATTR_SYSTEM: u32 = 0x4;
const ATTR_SPARSE: u32 = 0x200;
const ATTR_REPARSE_POINT: u32 = 0x400;
const ATTR_COMPRESSED: u32 = 0x800;

/// Name of the root node for a drive letter, e.g. `"C:"`.
pub(crate) fn root_name(letter: char) -> String {
    format!("{}:", letter.to_ascii_uppercase())
}

/// What one record slot of the MFT holds.
#[derive(Debug)]
pub(crate) enum Slot {
    /// An in-use record.
    Used(FileRecord),
    /// A never-used (all-zero) or deleted record.
    Free,
    /// A record that failed the fixup check or could not be parsed.
    Bad,
}

/// Applies fixups to one raw record slot (in place) and parses it.
pub(crate) fn parse_slot(bytes: &mut [u8], rec_no: u64, bytes_per_sector: u32) -> Slot {
    match bytes.get(..4) {
        None => return Slot::Bad,
        Some(magic) if magic == [0u8; 4] => return Slot::Free,
        Some(_) => {}
    }
    if apply_fixups(bytes, bytes_per_sector).is_err() {
        return Slot::Bad;
    }
    match parse_record(bytes, rec_no) {
        Ok(Some(record)) => Slot::Used(record),
        Ok(None) => Slot::Free,
        Err(_) => Slot::Bad,
    }
}

/// Counters from [`merge_records`].
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct MergeStats {
    /// Extension records whose base record was missing; their data is dropped.
    pub dropped_extensions: u64,
}

/// Merges extension records into their base records and emits one [`EntryRecord`] per name.
///
/// - An extension record (`base_record != 0`) adds its names, size and alloc to its base
///   record. If the base is missing (or is itself an extension), the extension is dropped and
///   counted in [`MergeStats::dropped_extensions`].
/// - The root (record 5) emits nothing: [`bf_core::TreeBuilder::new`] creates it.
/// - The first name of a record has key `rec_no` and carries the size. Every further name
///   `idx` (a hard link) has key `rec_no | idx << 48`, size 0 and the `HARDLINK` flag.
/// - Parent keys are plain record numbers. Records without names emit nothing.
///
/// Output order is deterministic: by record number, then by name order.
pub(crate) fn merge_records(
    mut records: Vec<FileRecord>,
    mut emit: impl FnMut(EntryRecord),
) -> MergeStats {
    let mut stats = MergeStats::default();
    records.sort_unstable_by_key(|r| r.rec_no);

    for i in 0..records.len() {
        let base_no = records[i].base_record;
        if base_no == 0 {
            continue;
        }
        let base_idx = records
            .binary_search_by_key(&base_no, |r| r.rec_no)
            .ok()
            .filter(|&j| records[j].base_record == 0);
        let Some(j) = base_idx else {
            stats.dropped_extensions += 1;
            continue;
        };
        let names = std::mem::take(&mut records[i].names);
        let (size, alloc) = (records[i].size, records[i].alloc);
        let base = &mut records[j];
        base.size = base.size.saturating_add(size);
        base.alloc = base.alloc.saturating_add(alloc);
        for name in names {
            let duplicate = base
                .names
                .iter()
                .any(|n| n.parent == name.parent && n.name == name.name);
            if !duplicate {
                base.names.push(name);
            }
        }
    }

    for record in records {
        if record.base_record != 0 || record.rec_no == ROOT_RECORD || record.rec_no > RECORD_MASK {
            continue;
        }
        emit_names(record, &mut emit);
    }
    stats
}

/// Emits one entry per name of a merged base record.
fn emit_names(record: FileRecord, emit: &mut impl FnMut(EntryRecord)) {
    let flags = flags_of(&record);
    for (idx, name) in record.names.into_iter().take(MAX_NAMES).enumerate() {
        let first = idx == 0;
        emit(EntryRecord {
            key: record.rec_no | ((idx as u64) << LINK_SHIFT),
            parent_key: name.parent & RECORD_MASK,
            name: name.name,
            size: if first { record.size } else { 0 },
            alloc: if first { record.alloc } else { 0 },
            mtime: record.mtime,
            flags: if first {
                flags
            } else {
                flags | NodeFlags::HARDLINK
            },
        });
    }
}

fn flags_of(record: &FileRecord) -> NodeFlags {
    let mut flags = if record.is_dir {
        NodeFlags::DIR
    } else {
        NodeFlags::empty()
    };
    for (bit, flag) in [
        (ATTR_HIDDEN, NodeFlags::HIDDEN),
        (ATTR_SYSTEM, NodeFlags::SYSTEM),
        (ATTR_REPARSE_POINT, NodeFlags::REPARSE),
        (ATTR_COMPRESSED, NodeFlags::COMPRESSED),
        (ATTR_SPARSE, NodeFlags::SPARSE),
    ] {
        if record.file_attributes & bit != 0 {
            flags |= flag;
        }
    }
    flags
}

#[cfg(test)]
pub(crate) mod tests {
    use bf_core::TreeBuilder;
    use bf_ntfs::FileName;

    use super::*;

    fn name(parent: u64, name: &str) -> FileName {
        FileName {
            parent,
            namespace: 1,
            name: name.to_string(),
        }
    }

    fn file(rec_no: u64, names: Vec<FileName>, size: u64, alloc: u64) -> FileRecord {
        FileRecord {
            rec_no,
            base_record: 0,
            is_dir: false,
            mtime: 1000 + rec_no as i64,
            file_attributes: 0,
            names,
            size,
            alloc,
            mft_runs: None,
        }
    }

    fn dir(rec_no: u64, parent: u64, n: &str) -> FileRecord {
        FileRecord {
            is_dir: true,
            ..file(rec_no, vec![name(parent, n)], 0, 0)
        }
    }

    fn extension(
        rec_no: u64,
        base: u64,
        names: Vec<FileName>,
        size: u64,
        alloc: u64,
    ) -> FileRecord {
        FileRecord {
            base_record: base,
            mtime: 0,
            ..file(rec_no, names, size, alloc)
        }
    }

    fn merge(records: Vec<FileRecord>) -> (Vec<EntryRecord>, MergeStats) {
        let mut out = Vec::new();
        let stats = merge_records(records, |e| out.push(e));
        (out, stats)
    }

    #[test]
    fn root_name_is_upper_case_letter_and_colon() {
        assert_eq!(root_name('c'), "C:");
        assert_eq!(root_name('D'), "D:");
    }

    #[test]
    fn plain_file_becomes_one_entry() {
        let (out, stats) = merge(vec![file(40, vec![name(5, "a.txt")], 10, 4096)]);
        assert_eq!(stats, MergeStats::default());
        assert_eq!(out.len(), 1);
        let e = &out[0];
        assert_eq!((e.key, e.parent_key, e.name.as_str()), (40, 5, "a.txt"));
        assert_eq!((e.size, e.alloc, e.mtime), (10, 4096, 1040));
        assert_eq!(e.flags, NodeFlags::empty());
    }

    #[test]
    fn root_record_and_its_self_name_are_skipped() {
        let root = FileRecord {
            is_dir: true,
            ..file(ROOT_RECORD, vec![name(ROOT_RECORD, ".")], 0, 0)
        };
        let (out, _) = merge(vec![root, dir(30, ROOT_RECORD, "Windows")]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "Windows");
        assert!(out[0].flags.contains(NodeFlags::DIR));
    }

    #[test]
    fn extension_adds_names_and_sizes_to_its_base() {
        let base = file(50, vec![name(5, "big.bin")], 100, 4096);
        let ext = extension(51, 50, vec![name(30, "link.bin")], 900, 8192);
        // Extension first: input order must not matter.
        let (out, stats) = merge(vec![ext, base]);
        assert_eq!(stats.dropped_extensions, 0);
        assert_eq!(out.len(), 2);
        assert_eq!((out[0].key, out[0].size, out[0].alloc), (50, 1000, 12288));
        assert_eq!(out[0].name, "big.bin");
        assert_eq!((out[1].key, out[1].parent_key), (50 | 1 << 48, 30));
        assert_eq!((out[1].size, out[1].alloc), (0, 0));
        assert!(out[1].flags.contains(NodeFlags::HARDLINK));
        // The base's mtime and attributes are kept, not the extension's zeros.
        assert_eq!(out[1].mtime, 1050);
    }

    #[test]
    fn extension_sizes_saturate() {
        let base = file(50, vec![name(5, "huge")], u64::MAX - 1, u64::MAX);
        let ext = extension(51, 50, vec![], 10, 10);
        let (out, _) = merge(vec![base, ext]);
        assert_eq!((out[0].size, out[0].alloc), (u64::MAX, u64::MAX));
    }

    #[test]
    fn extension_without_base_is_dropped_and_counted() {
        let ext = extension(70, 69, vec![name(5, "lost")], 500, 4096);
        // An extension pointing at another extension has no usable base either.
        let ext2 = extension(71, 70, vec![name(5, "lost2")], 1, 1);
        let (out, stats) = merge(vec![ext, ext2, file(80, vec![name(5, "ok")], 1, 0)]);
        assert_eq!(stats.dropped_extensions, 2);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "ok");
    }

    #[test]
    fn duplicate_name_from_extension_is_not_repeated() {
        let base = file(50, vec![name(5, "same")], 1, 0);
        let ext = extension(51, 50, vec![name(5, "same")], 0, 0);
        let (out, _) = merge(vec![base, ext]);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn hard_links_get_distinct_keys_and_only_first_has_size() {
        let mut rec = file(
            60,
            vec![name(5, "one"), name(30, "two"), name(31, "three")],
            300,
            4096,
        );
        rec.file_attributes = ATTR_HIDDEN;
        let (out, _) = merge(vec![rec]);
        let keys: Vec<u64> = out.iter().map(|e| e.key).collect();
        assert_eq!(keys, vec![60, 60 | 1 << 48, 60 | 2 << 48]);
        let parents: Vec<u64> = out.iter().map(|e| e.parent_key).collect();
        assert_eq!(parents, vec![5, 30, 31]);
        assert_eq!((out[0].size, out[0].alloc), (300, 4096));
        assert!(!out[0].flags.contains(NodeFlags::HARDLINK));
        for e in &out[1..] {
            assert_eq!((e.size, e.alloc), (0, 0));
            assert!(e.flags.contains(NodeFlags::HARDLINK | NodeFlags::HIDDEN));
        }
    }

    #[test]
    fn two_names_build_two_nodes_with_size_counted_once() {
        let records = vec![
            dir(30, ROOT_RECORD, "a"),
            dir(31, ROOT_RECORD, "b"),
            file(60, vec![name(30, "x"), name(31, "y")], 100, 4096),
        ];
        let mut builder = TreeBuilder::new(ROOT_RECORD, &root_name('c'));
        merge_records(records, |e| builder.push(e));
        let tree = builder.finish();
        let total = tree.total(tree.root());
        assert_eq!((total.files, total.dirs, total.size), (2, 2, 100));
    }

    #[test]
    fn records_without_names_emit_nothing() {
        let (out, _) = merge(vec![file(90, vec![], 10, 0)]);
        assert!(out.is_empty());
    }

    #[test]
    fn attribute_bits_map_to_flags() {
        let mut rec = file(61, vec![name(5, "f")], 0, 0);
        rec.file_attributes =
            ATTR_HIDDEN | ATTR_SYSTEM | ATTR_REPARSE_POINT | ATTR_COMPRESSED | ATTR_SPARSE | 0x20;
        let (out, _) = merge(vec![rec]);
        assert_eq!(
            out[0].flags,
            NodeFlags::HIDDEN
                | NodeFlags::SYSTEM
                | NodeFlags::REPARSE
                | NodeFlags::COMPRESSED
                | NodeFlags::SPARSE
        );
    }

    #[test]
    fn output_is_sorted_by_record_number() {
        let (out, _) = merge(vec![
            file(9, vec![name(5, "c")], 0, 0),
            file(7, vec![name(5, "a")], 0, 0),
            file(8, vec![name(5, "b")], 0, 0),
        ]);
        let names: Vec<&str> = out.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    /// Builds a minimal FILE record of `size` bytes with a fixup array for 512-byte sectors
    /// and only the end marker as attributes.
    pub(crate) fn raw_record(size: usize, in_use: bool) -> Vec<u8> {
        let sectors = size / 512;
        let mut r = vec![0u8; size];
        r[0..4].copy_from_slice(b"FILE");
        r[4..6].copy_from_slice(&48u16.to_le_bytes()); // USA offset
        r[6..8].copy_from_slice(&((sectors + 1) as u16).to_le_bytes()); // USA count
        r[20..22].copy_from_slice(&56u16.to_le_bytes()); // first attribute
        r[22..24].copy_from_slice(&(u16::from(in_use)).to_le_bytes()); // flags
        r[24..28].copy_from_slice(&64u32.to_le_bytes()); // used size
        r[56..60].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // end marker
        // Update sequence number 0x0001; every sector tail carries it.
        r[48..50].copy_from_slice(&1u16.to_le_bytes());
        for s in 1..=sectors {
            r[s * 512 - 2..s * 512].copy_from_slice(&1u16.to_le_bytes());
        }
        r
    }

    #[test]
    fn parse_slot_classifies_records() {
        let mut zero = vec![0u8; 1024];
        assert!(matches!(parse_slot(&mut zero, 3, 512), Slot::Free));

        let mut used = raw_record(1024, true);
        match parse_slot(&mut used, 42, 512) {
            Slot::Used(r) => assert_eq!(r.rec_no, 42),
            other => panic!("expected Used, got {other:?}"),
        }

        let mut deleted = raw_record(1024, false);
        assert!(matches!(parse_slot(&mut deleted, 43, 512), Slot::Free));

        let mut torn = raw_record(1024, true);
        torn[1022] = 0x99; // sector tail no longer matches the update sequence number
        assert!(matches!(parse_slot(&mut torn, 44, 512), Slot::Bad));

        let mut garbage = vec![0xABu8; 1024];
        assert!(matches!(parse_slot(&mut garbage, 45, 512), Slot::Bad));

        let mut short = vec![1u8; 2];
        assert!(matches!(parse_slot(&mut short, 46, 512), Slot::Bad));
    }
}
