mod support;

use bf_ntfs::{NtfsError, apply_fixups, parse_record};
use support::record_builder::{NonResident, RecordBuilder};

const FILETIME: i64 = 133_000_000_000_000_000;
/// Offset of the first attribute in builder output (1 KiB record).
const FIRST: usize = 0x38;

/// One run: 16 clusters at LCN 256.
const RUNS: [u8; 5] = [0x21, 0x10, 0x00, 0x01, 0x00];

fn set16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

fn set32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// Builds a protected record, lets the test corrupt it, then removes the
/// protection (corruptions must not touch the sector tails).
fn corrupted(b: &RecordBuilder, corrupt: impl FnOnce(&mut [u8])) -> Vec<u8> {
    let mut rec = b.build(1024, 512);
    corrupt(&mut rec);
    apply_fixups(&mut rec, 512).unwrap();
    rec
}

#[test]
fn file_with_win32_and_dos_names_keeps_win32_only() {
    let rec = RecordBuilder::new(40)
        .std_info(FILETIME, 0x20)
        .file_name(5, 2, "LONGFI~1.TXT")
        .file_name(5, 1, "LongFileName.txt")
        .build_plain();
    let f = parse_record(&rec, 40).unwrap().unwrap();
    assert_eq!(f.names.len(), 1);
    assert_eq!(f.names[0].name, "LongFileName.txt");
    assert_eq!(f.names[0].namespace, 1);
    assert_eq!(f.names[0].parent, 5);
    assert_eq!(f.rec_no, 40);
    assert_eq!(f.mtime, FILETIME);
    assert_eq!(f.file_attributes, 0x20);
    assert!(!f.is_dir);
    assert_eq!(f.mft_runs, None);
}

#[test]
fn posix_and_win32_dos_namespaces_are_kept() {
    let rec = RecordBuilder::new(40)
        .file_name(5, 0, "posix")
        .file_name(6, 3, "same.txt")
        .build_plain();
    let f = parse_record(&rec, 40).unwrap().unwrap();
    assert_eq!(f.names.len(), 2);
}

#[test]
fn parent_reference_is_masked_to_48_bits() {
    // Upper 16 bits carry the parent's sequence number.
    let rec = RecordBuilder::new(40)
        .file_name(0x0007_0000_0000_002A, 1, "a")
        .build_plain();
    let f = parse_record(&rec, 40).unwrap().unwrap();
    assert_eq!(f.names[0].parent, 42);
}

#[test]
fn hardlink_two_win32_names_different_parents() {
    let rec = RecordBuilder::new(40)
        .link_count(2)
        .file_name(5, 1, "a.txt")
        .file_name(9, 1, "b.txt")
        .build_plain();
    let f = parse_record(&rec, 40).unwrap().unwrap();
    assert_eq!(f.names.len(), 2);
    assert_eq!(f.names[0].parent, 5);
    assert_eq!(f.names[1].parent, 9);
}

#[test]
fn duplicate_parent_and_name_is_deduplicated() {
    let rec = RecordBuilder::new(40)
        .file_name(5, 1, "a.txt")
        .file_name(5, 3, "a.txt")
        .build_plain();
    let f = parse_record(&rec, 40).unwrap().unwrap();
    assert_eq!(f.names.len(), 1);
}

#[test]
fn resident_data_size() {
    let rec = RecordBuilder::new(40)
        .file_name(5, 1, "tiny")
        .data_resident(100)
        .build_plain();
    let f = parse_record(&rec, 40).unwrap().unwrap();
    assert_eq!(f.size, 100);
    assert_eq!(f.alloc, 0);
}

#[test]
fn nonresident_data_alloc_and_real() {
    let rec = RecordBuilder::new(40)
        .file_name(5, 1, "big")
        .data_nonresident(65_536, 60_000, &RUNS)
        .build_plain();
    let f = parse_record(&rec, 40).unwrap().unwrap();
    assert_eq!(f.size, 60_000);
    assert_eq!(f.alloc, 65_536);
    assert_eq!(f.mft_runs, None); // only record 0 reports runs
}

#[test]
fn compressed_uses_compressed_size() {
    let rec = RecordBuilder::new(40)
        .data_nonresident_with(NonResident::new(65_536, 60_000, &RUNS).compressed(16_384))
        .build_plain();
    let f = parse_record(&rec, 40).unwrap().unwrap();
    assert_eq!(f.size, 60_000);
    assert_eq!(f.alloc, 16_384);
}

#[test]
fn sparse_uses_compressed_size() {
    let rec = RecordBuilder::new(40)
        .data_nonresident_with(NonResident::new(1 << 20, 1 << 20, &RUNS).sparse(4096))
        .build_plain();
    let f = parse_record(&rec, 40).unwrap().unwrap();
    assert_eq!(f.size, 1 << 20);
    assert_eq!(f.alloc, 4096);
}

#[test]
fn compressed_without_compressed_size_field_is_error() {
    // Flag says compressed, but the attribute header is the short 64-byte kind.
    let b = RecordBuilder::new(40).data_nonresident(65_536, 60_000, &RUNS);
    let rec = corrupted(&b, |r| r[FIRST + 12] |= 0x01);
    assert_eq!(parse_record(&rec, 40), Err(NtfsError::BadAttribute(FIRST)));
}

#[test]
fn ads_sizes_are_summed() {
    let rec = RecordBuilder::new(40)
        .file_name(5, 1, "download.exe")
        .data_resident(100)
        .data_resident_named("Zone.Identifier", 26)
        .build_plain();
    let f = parse_record(&rec, 40).unwrap().unwrap();
    assert_eq!(f.size, 126);
}

#[test]
fn later_nonresident_pieces_do_not_add_sizes() {
    // An extension record continues a stream at VCN 16; its size fields are
    // undefined and must be ignored.
    let rec = RecordBuilder::new(41)
        .base(40)
        .data_nonresident_with(NonResident::new(999_999, 888_888, &RUNS).start_vcn(16))
        .build_plain();
    let f = parse_record(&rec, 41).unwrap().unwrap();
    assert_eq!(f.size, 0);
    assert_eq!(f.alloc, 0);
}

#[test]
fn negative_size_is_error() {
    let rec = RecordBuilder::new(40)
        .data_nonresident(u64::MAX, 10, &RUNS) // -1 as i64
        .build_plain();
    assert_eq!(parse_record(&rec, 40), Err(NtfsError::BadAttribute(FIRST)));
}

#[test]
fn unused_record_returns_none() {
    let rec = RecordBuilder::new(40)
        .in_use(false)
        .file_name(5, 1, "gone")
        .build_plain();
    assert_eq!(parse_record(&rec, 40), Ok(None));
}

#[test]
fn directory_flag_is_reported() {
    let rec = RecordBuilder::new(40)
        .dir(true)
        .file_name(5, 1, "dir")
        .build_plain();
    assert!(parse_record(&rec, 40).unwrap().unwrap().is_dir);
}

#[test]
fn extension_record_reports_base() {
    let rec = RecordBuilder::new(90)
        .base(77)
        .data_nonresident_with(NonResident::new(0, 0, &RUNS).start_vcn(8))
        .build_plain();
    let f = parse_record(&rec, 90).unwrap().unwrap();
    assert_eq!(f.base_record, 77);
    assert_eq!(f.mtime, 0);
    assert_eq!(f.file_attributes, 0);
    assert!(f.names.is_empty());
}

#[test]
fn mft_runs_reported_only_for_record_zero() {
    let rec = RecordBuilder::new(0)
        .std_info(FILETIME, 6)
        .file_name(5, 3, "$MFT")
        .data_nonresident(4096 * 16, 4096 * 16, &RUNS)
        .build_plain();
    let f = parse_record(&rec, 0).unwrap().unwrap();
    let runs = f.mft_runs.expect("record 0 reports runs");
    assert_eq!(runs.len(), 1);
    assert_eq!((runs[0].lcn, runs[0].clusters), (256, 16));
}

#[test]
fn mft_runs_ignore_named_streams() {
    let rec = RecordBuilder::new(0)
        .data_nonresident_with(NonResident::new(4096, 4096, &RUNS).named("other"))
        .build_plain();
    let f = parse_record(&rec, 0).unwrap().unwrap();
    assert_eq!(f.mft_runs, None);
    assert_eq!(f.size, 4096); // the stream is still counted
}

#[test]
fn lone_surrogate_name_is_lossy() {
    let rec = RecordBuilder::new(40)
        .file_name_utf16(5, 1, &[0xD800, 0x0041])
        .build_plain();
    let f = parse_record(&rec, 40).unwrap().unwrap();
    assert_eq!(f.names[0].name, "\u{FFFD}A");
}

#[test]
fn unknown_attributes_are_skipped() {
    // An $ATTRIBUTE_LIST (0x20) and an unknown type with arbitrary bodies.
    let mut list = vec![0u8; 32];
    set32(&mut list, 0, 0x20);
    set32(&mut list, 4, 32);
    let mut odd = vec![0u8; 24];
    set32(&mut odd, 0, 0xB0);
    set32(&mut odd, 4, 24);
    let rec = RecordBuilder::new(40)
        .raw_attr(list)
        .raw_attr(odd)
        .file_name(5, 1, "x")
        .build_plain();
    let f = parse_record(&rec, 40).unwrap().unwrap();
    assert_eq!(f.names.len(), 1);
}

#[test]
fn zero_length_attribute_is_error() {
    let mut bad = vec![0u8; 24];
    set32(&mut bad, 0, 0x80); // type set, length left 0
    let rec = RecordBuilder::new(40).raw_attr(bad).build_plain();
    assert_eq!(parse_record(&rec, 40), Err(NtfsError::BadAttribute(FIRST)));
}

#[test]
fn attribute_past_used_size_is_error() {
    let b = RecordBuilder::new(40).file_name(5, 1, "abc");
    // The attribute claims to be larger than the used part of the record.
    let rec = corrupted(&b, |r| set32(r, FIRST + 4, 0x0800));
    assert_eq!(parse_record(&rec, 40), Err(NtfsError::BadAttribute(FIRST)));
}

#[test]
fn used_size_beyond_buffer_is_clamped() {
    let mut rec = RecordBuilder::new(40).file_name(5, 1, "abc").build_plain();
    set32(&mut rec, 24, 0x00FF_FFFF); // used_size far beyond the buffer
    // Still parses: the end marker is found inside the real buffer.
    assert_eq!(parse_record(&rec, 40).unwrap().unwrap().names.len(), 1);
    // And an attribute running off the end of the buffer is an error.
    set32(&mut rec, FIRST + 4, 0x0FFF);
    assert_eq!(parse_record(&rec, 40), Err(NtfsError::BadAttribute(FIRST)));
}

#[test]
fn first_attribute_offset_out_of_range_is_error() {
    let mut rec = RecordBuilder::new(40).file_name(5, 1, "abc").build_plain();
    for off in [0u16, 8, 41, 0x0400, 0xFFFF] {
        set16(&mut rec, 20, off);
        assert_eq!(
            parse_record(&rec, 40),
            Err(NtfsError::BadAttribute(usize::from(off))),
            "first_attr_offset {off}"
        );
    }
}

#[test]
fn missing_end_marker_is_error() {
    let mut rec = RecordBuilder::new(40).file_name(5, 1, "abc").build_plain();
    // Shrink used_size so that the end marker falls outside it.
    let attr_len = u32::from_le_bytes(rec[FIRST + 4..FIRST + 8].try_into().unwrap());
    set32(&mut rec, 24, FIRST as u32 + attr_len);
    assert!(matches!(
        parse_record(&rec, 40),
        Err(NtfsError::BadAttribute(_))
    ));
}

#[test]
fn malformed_file_name_value_is_error() {
    let mut rec = RecordBuilder::new(40).file_name(5, 1, "abc").build_plain();
    // name_len (value offset 64) claims more UTF-16 units than the value holds.
    rec[FIRST + 24 + 64] = 200;
    assert_eq!(parse_record(&rec, 40), Err(NtfsError::BadAttribute(FIRST)));
}

#[test]
fn runs_offset_out_of_range_is_error() {
    let b = RecordBuilder::new(0).data_nonresident(4096, 4096, &RUNS);
    let rec = corrupted(&b, |r| set16(r, FIRST + 32, 0x7000));
    assert_eq!(parse_record(&rec, 0), Err(NtfsError::BadAttribute(FIRST)));
}

#[test]
fn bad_magic_and_short_buffers_are_errors() {
    let rec = RecordBuilder::new(40).build_plain();
    assert_eq!(parse_record(&rec[..10], 40), Err(NtfsError::Truncated));
    let mut junk = rec.clone();
    junk[0..4].copy_from_slice(b"BAAD");
    assert_eq!(parse_record(&junk, 40), Err(NtfsError::BadMagic));
}

#[test]
fn garbage_never_panics() {
    let rec = RecordBuilder::new(0)
        .std_info(FILETIME, 0x20)
        .file_name(5, 1, "name")
        .data_resident(10)
        .data_nonresident(4096, 4000, &RUNS)
        .build_plain();
    // Truncate at every length.
    for len in 0..=rec.len() {
        let _ = parse_record(&rec[..len], 0);
    }
    // Overwrite each byte of the attribute area with a few hostile values.
    for pos in FIRST..0x1A0 {
        for v in [0x00u8, 0x01, 0x7F, 0x80, 0xFF] {
            let mut bad = rec.clone();
            bad[pos] = v;
            let _ = parse_record(&bad, 0);
        }
    }
    // Constant fill after a valid magic, flagged as in use.
    for fill in [0x00u8, 0x7F, 0xFF] {
        let mut bad = vec![fill; 1024];
        bad[0..4].copy_from_slice(b"FILE");
        set16(&mut bad, 22, 1);
        let _ = parse_record(&bad, 0);
    }
}
