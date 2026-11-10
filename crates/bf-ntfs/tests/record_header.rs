mod support;

use bf_ntfs::{NtfsError, apply_fixups, parse_header, parse_volume_data};
use support::record_builder::{RecordBuilder, USN};

const REC: usize = 1024;
const SECTOR: usize = 512;
/// Offset of the update sequence array in builder output.
const USA: usize = 0x30;

fn le16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn set16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

fn volume_buf() -> Vec<u8> {
    let mut b = vec![0u8; 96];
    b[0..8].copy_from_slice(&0x1122_3344_5566_7788i64.to_le_bytes()); // serial
    b[16..24].copy_from_slice(&1_000_000i64.to_le_bytes()); // total clusters
    b[40..44].copy_from_slice(&512u32.to_le_bytes());
    b[44..48].copy_from_slice(&4096u32.to_le_bytes());
    b[48..52].copy_from_slice(&1024u32.to_le_bytes());
    b[56..64].copy_from_slice(&262_144i64.to_le_bytes()); // MFT valid length
    b[64..72].copy_from_slice(&786_432i64.to_le_bytes()); // MFT start LCN
    b
}

#[test]
fn parses_volume_data() {
    let v = parse_volume_data(&volume_buf()).unwrap();
    assert_eq!(v.serial, 0x1122_3344_5566_7788);
    assert_eq!(v.total_clusters, 1_000_000);
    assert_eq!(v.bytes_per_sector, 512);
    assert_eq!(v.bytes_per_cluster, 4096);
    assert_eq!(v.bytes_per_record, 1024);
    assert_eq!(v.mft_valid_data_length, 262_144);
    assert_eq!(v.mft_start_lcn, 786_432);
}

#[test]
fn volume_data_too_short_is_error() {
    let buf = volume_buf();
    assert_eq!(parse_volume_data(&buf[..71]), Err(NtfsError::Truncated));
    assert_eq!(parse_volume_data(&[]), Err(NtfsError::Truncated));
    assert!(parse_volume_data(&buf[..72]).is_ok());
}

#[test]
fn volume_data_negative_field_is_error() {
    let mut buf = volume_buf();
    buf[16..24].copy_from_slice(&(-1i64).to_le_bytes());
    assert_eq!(parse_volume_data(&buf), Err(NtfsError::Truncated));
}

#[test]
fn builder_output_is_protected() {
    let rec = RecordBuilder::new(7).build(REC, SECTOR);
    assert_eq!(&rec[0..4], b"FILE");
    assert_eq!(le16(&rec, 4), USA as u16);
    assert_eq!(le16(&rec, 6), 3);
    assert_eq!(le16(&rec, USA), USN);
    assert_eq!(le16(&rec, 510), USN);
    assert_eq!(le16(&rec, 1022), USN);
    assert_eq!(le16(&rec, 20), 0x38); // first attribute, 8-aligned after USA
    assert_eq!(&rec[0x38..0x3C], &[0xFF; 4]); // end marker
    assert_eq!(u32::from_le_bytes(rec[24..28].try_into().unwrap()), 0x40);
    assert_eq!(u32::from_le_bytes(rec[28..32].try_into().unwrap()), 1024);
}

#[test]
fn fixups_roundtrip() {
    let mut rec = RecordBuilder::new(7).build(REC, SECTOR);
    // Pretend the original sector tails were non-zero.
    set16(&mut rec, USA + 2, 0xABCD);
    set16(&mut rec, USA + 4, 0x1234);
    apply_fixups(&mut rec, SECTOR as u32).unwrap();
    assert_eq!(le16(&rec, 510), 0xABCD);
    assert_eq!(le16(&rec, 1022), 0x1234);
}

#[test]
fn fixup_mismatch_detected() {
    let mut rec = RecordBuilder::new(7).build(REC, SECTOR);
    set16(&mut rec, 1022, 0xDEAD); // sector 2 tail
    assert_eq!(
        apply_fixups(&mut rec, SECTOR as u32),
        Err(NtfsError::FixupMismatch)
    );
}

#[test]
fn failed_fixup_leaves_record_untouched() {
    let mut rec = RecordBuilder::new(7).build(REC, SECTOR);
    set16(&mut rec, USA + 2, 0xABCD);
    set16(&mut rec, 1022, 0xDEAD); // second sector is bad, first is fine
    let before = rec.clone();
    assert!(apply_fixups(&mut rec, SECTOR as u32).is_err());
    assert_eq!(rec, before);
}

#[test]
fn bad_magic() {
    let mut rec = RecordBuilder::new(7).build(REC, SECTOR);
    rec[0..4].copy_from_slice(b"BAAD");
    assert_eq!(parse_header(&rec), Err(NtfsError::BadMagic));
    assert_eq!(
        apply_fixups(&mut rec, SECTOR as u32),
        Err(NtfsError::BadMagic)
    );
    rec[0..4].copy_from_slice(b"JUNK");
    assert_eq!(parse_header(&rec), Err(NtfsError::BadMagic));
}

#[test]
fn usa_count_out_of_range_is_error() {
    let mut rec = RecordBuilder::new(7).build(REC, SECTOR);
    set16(&mut rec, 6, 200);
    assert!(apply_fixups(&mut rec, SECTOR as u32).is_err());
    set16(&mut rec, 6, 0);
    assert!(apply_fixups(&mut rec, SECTOR as u32).is_err());
    set16(&mut rec, 6, 0xFFFF);
    assert!(apply_fixups(&mut rec, SECTOR as u32).is_err());
}

#[test]
fn usa_offset_out_of_range_is_error() {
    let mut rec = RecordBuilder::new(7).build(REC, SECTOR);
    set16(&mut rec, 4, 1022); // USA would run past the end
    assert_eq!(
        apply_fixups(&mut rec, SECTOR as u32),
        Err(NtfsError::Truncated)
    );
    set16(&mut rec, 4, 0xFFFF);
    assert_eq!(
        apply_fixups(&mut rec, SECTOR as u32),
        Err(NtfsError::Truncated)
    );
}

#[test]
fn bad_sector_size_is_error() {
    let mut rec = RecordBuilder::new(7).build(REC, SECTOR);
    for bps in [0u32, 1, 300, 2048] {
        assert!(apply_fixups(&mut rec, bps).is_err(), "bps {bps}");
    }
}

#[test]
fn truncated_inputs_never_panic() {
    let rec = RecordBuilder::new(7).build(REC, SECTOR);
    for len in 0..=rec.len() {
        let mut part = rec[..len].to_vec();
        let _ = parse_header(&part);
        let _ = apply_fixups(&mut part, SECTOR as u32);
    }
}

#[test]
fn garbage_header_never_panics() {
    // Valid magic followed by constant fill in every header field.
    for fill in [0x00u8, 0x7F, 0xFF] {
        let mut rec = vec![fill; REC];
        rec[0..4].copy_from_slice(b"FILE");
        let _ = parse_header(&rec);
        let _ = apply_fixups(&mut rec, SECTOR as u32);
    }
}

#[test]
fn header_flags() {
    let rec = RecordBuilder::new(5)
        .dir(true)
        .in_use(false)
        .link_count(3)
        .base(0x0003_0000_0000_0005)
        .build(REC, SECTOR);
    let h = parse_header(&rec).unwrap();
    assert!(h.is_dir);
    assert!(!h.in_use);
    assert_eq!(h.link_count, 3);
    assert_eq!(h.base_record, 5); // upper 16 bits (sequence) masked off
    assert_eq!(h.first_attr_offset, 0x38);
    assert_eq!(h.used_size, 0x40);

    let rec = RecordBuilder::new(6).build(REC, SECTOR);
    let h = parse_header(&rec).unwrap();
    assert!(h.in_use);
    assert!(!h.is_dir);
    assert_eq!(h.base_record, 0);
    assert_eq!(h.link_count, 1);
}

#[test]
fn header_too_short_is_error() {
    let rec = RecordBuilder::new(5).build(REC, SECTOR);
    assert_eq!(parse_header(&rec[..39]), Err(NtfsError::Truncated));
    assert!(parse_header(&rec[..40]).is_ok());
}
