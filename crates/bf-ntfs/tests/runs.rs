use bf_ntfs::{Extent, NtfsError, decode_runs};

#[test]
fn decode_simple_run() {
    let runs = decode_runs(&[0x21, 0x10, 0x00, 0x01, 0x00]).unwrap();
    assert_eq!(
        runs,
        [Extent {
            lcn: 256,
            clusters: 16
        }]
    );
}

#[test]
fn decode_negative_relative_offset() {
    // Run 1: 16 clusters at LCN 0x100. Run 2: 8 clusters at 0x100 - 16.
    let buf = [0x21, 0x10, 0x00, 0x01, 0x11, 0x08, 0xF0, 0x00];
    let runs = decode_runs(&buf).unwrap();
    assert_eq!(
        runs,
        [
            Extent {
                lcn: 256,
                clusters: 16
            },
            Extent {
                lcn: 240,
                clusters: 8
            },
        ]
    );
}

#[test]
fn sparse_run_skipped() {
    // 4 clusters at LCN 16, a 100-cluster hole, then 4 clusters at 16 + 4.
    let buf = [0x11, 0x04, 0x10, 0x01, 0x64, 0x11, 0x04, 0x04, 0x00];
    let runs = decode_runs(&buf).unwrap();
    assert_eq!(
        runs,
        [
            Extent {
                lcn: 16,
                clusters: 4
            },
            Extent {
                lcn: 20,
                clusters: 4
            },
        ]
    );
}

#[test]
fn empty_run_list_is_ok() {
    assert_eq!(decode_runs(&[0x00]).unwrap(), []);
}

#[test]
fn truncated_run_is_error() {
    // Header promises 1 length byte + 2 offset bytes, only 1 offset byte left.
    assert_eq!(decode_runs(&[0x21, 0x10, 0x00]), Err(NtfsError::Truncated));
    assert_eq!(decode_runs(&[0x21]), Err(NtfsError::Truncated));
    // No 0x00 terminator.
    assert_eq!(
        decode_runs(&[0x21, 0x10, 0x00, 0x01]),
        Err(NtfsError::Truncated)
    );
    assert_eq!(decode_runs(&[]), Err(NtfsError::Truncated));
}

#[test]
fn invalid_field_sizes_are_errors() {
    // length_size 0.
    assert!(decode_runs(&[0x10, 0x01, 0x00]).is_err());
    // length_size 9.
    let mut buf = vec![0x19];
    buf.extend_from_slice(&[1; 10]);
    buf.push(0);
    assert!(decode_runs(&buf).is_err());
    // offset_size 9.
    let mut buf = vec![0x91, 0x01];
    buf.extend_from_slice(&[1; 9]);
    buf.push(0);
    assert!(decode_runs(&buf).is_err());
}

#[test]
fn negative_lcn_is_error() {
    // First run starts at LCN -1.
    assert_eq!(
        decode_runs(&[0x11, 0x01, 0xFF, 0x00]),
        Err(NtfsError::Truncated)
    );
}

#[test]
fn lcn_overflow_is_error() {
    // Two 8-byte offsets of i64::MAX overflow when added.
    let mut buf = vec![0x81, 0x01];
    buf.extend_from_slice(&i64::MAX.to_le_bytes());
    buf.extend_from_slice(&[0x81, 0x01]);
    buf.extend_from_slice(&i64::MAX.to_le_bytes());
    buf.push(0);
    assert_eq!(decode_runs(&buf), Err(NtfsError::Truncated));
}

#[test]
fn wide_fields_decode() {
    // 8-byte length and 8-byte offset.
    let mut buf = vec![0x88];
    buf.extend_from_slice(&0x1_0000_0000u64.to_le_bytes());
    buf.extend_from_slice(&0x2_0000_0000i64.to_le_bytes());
    buf.push(0);
    assert_eq!(
        decode_runs(&buf).unwrap(),
        [Extent {
            lcn: 0x2_0000_0000,
            clusters: 0x1_0000_0000
        }]
    );
}

#[test]
fn garbage_never_panics() {
    for fill in [0x00u8, 0x01, 0x7F, 0x88, 0xFF] {
        for len in 0..40 {
            let _ = decode_runs(&vec![fill; len]);
        }
    }
}
