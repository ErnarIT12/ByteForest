//! Where the MFT lives on the volume and how to read it: geometry checks, `$MFT` extents and
//! the plan of sector-aligned block reads. Pure logic without I/O, so it is unit-tested.

use bf_ntfs::{Extent, VolumeInfo};

use crate::ScanError;

/// Target size of one read block (8 MiB).
const BLOCK_TARGET: u64 = 8 << 20;
/// Largest MFT record size we accept (64 KiB).
const MAX_RECORD: u64 = 64 << 10;
/// Size of the `RETRIEVAL_POINTERS_BUFFER` header: `ExtentCount`, padding, `StartingVcn`.
const RP_HEADER: usize = 16;
/// Size of one `{ NextVcn, Lcn }` pair.
const RP_PAIR: usize = 16;
/// Size of the `NTFS_FILE_RECORD_OUTPUT_BUFFER` header: reference number and record length.
const FR_HEADER: usize = 12;

fn layout_error(msg: impl Into<String>) -> ScanError {
    ScanError::BadLayout(msg.into())
}

/// Validated sizes of an NTFS volume, all in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Geometry {
    pub sector: u64,
    pub cluster: u64,
    pub record: u64,
    /// Bytes of the MFT that hold valid data.
    pub mft_valid_len: u64,
    pub total_clusters: u64,
}

impl Geometry {
    /// Checks the sizes reported by `FSCTL_GET_NTFS_VOLUME_DATA` (bf-ntfs does not).
    pub fn new(info: &VolumeInfo) -> Result<Self, ScanError> {
        let sector = u64::from(info.bytes_per_sector);
        let cluster = u64::from(info.bytes_per_cluster);
        let record = u64::from(info.bytes_per_record);
        if sector == 0 || cluster == 0 || record == 0 {
            return Err(layout_error(format!(
                "zero size: sector {sector}, cluster {cluster}, record {record}"
            )));
        }
        if !sector.is_power_of_two() {
            return Err(layout_error(format!(
                "sector size {sector} is not a power of two"
            )));
        }
        if record > MAX_RECORD || !record.is_multiple_of(sector) {
            return Err(layout_error(format!(
                "record size {record} is not a whole number of {sector}-byte sectors up to 64 KiB"
            )));
        }
        if !cluster.is_multiple_of(sector) {
            return Err(layout_error(format!(
                "cluster size {cluster} is not a whole number of {sector}-byte sectors"
            )));
        }
        if info.total_clusters == 0 {
            return Err(layout_error("volume has no clusters"));
        }
        Ok(Self {
            sector,
            cluster,
            record,
            mft_valid_len: info.mft_valid_data_length,
            total_clusters: info.total_clusters,
        })
    }

    /// Bytes per sector as the `u32` that `apply_fixups` takes (validated to fit).
    pub fn sector_u32(&self) -> u32 {
        u32::try_from(self.sector).unwrap_or(u32::MAX)
    }
}

/// One run of `$MFT` clusters: `clusters` clusters at volume cluster `lcn`, holding the MFT's
/// virtual clusters starting at `vcn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MftExtent {
    pub vcn: u64,
    pub lcn: u64,
    pub clusters: u64,
}

/// Parses a `RETRIEVAL_POINTERS_BUFFER` (output of `FSCTL_GET_RETRIEVAL_POINTERS`).
///
/// Returns the extents and the VCN after the last one (where a follow-up call continues).
/// Pairs with `Lcn == -1` are unallocated: they produce no extent but still advance the VCN.
pub(crate) fn parse_retrieval_pointers(buf: &[u8]) -> Result<(Vec<MftExtent>, u64), ScanError> {
    let corrupt = || layout_error("corrupt retrieval pointers buffer");
    let count = usize::try_from(read_u32(buf, 0).ok_or_else(corrupt)?).map_err(|_| corrupt())?;
    let start = read_i64(buf, 8).ok_or_else(corrupt)?;
    let mut vcn = u64::try_from(start).map_err(|_| corrupt())?;

    let mut extents = Vec::with_capacity(count.min(buf.len() / RP_PAIR));
    for i in 0..count {
        let at = i
            .checked_mul(RP_PAIR)
            .and_then(|o| o.checked_add(RP_HEADER))
            .ok_or_else(corrupt)?;
        let next_vcn = read_i64(buf, at).ok_or_else(corrupt)?;
        let lcn = read_i64(buf, at + 8).ok_or_else(corrupt)?;
        let next_vcn = u64::try_from(next_vcn).map_err(|_| corrupt())?;
        if next_vcn < vcn {
            return Err(corrupt());
        }
        match lcn {
            -1 => {} // unallocated: no clusters on disk, but the VCN still advances
            lcn if lcn >= 0 => extents.push(MftExtent {
                vcn,
                lcn: lcn as u64,
                clusters: next_vcn - vcn,
            }),
            _ => return Err(corrupt()),
        }
        vcn = next_vcn;
    }
    Ok((extents, vcn))
}

/// Turns the data runs of record 0 into extents. `decode_runs` drops sparse runs, so the
/// runs are assumed to be contiguous from VCN 0 (the MFT is never sparse).
pub(crate) fn extents_from_runs(runs: &[Extent]) -> Vec<MftExtent> {
    let mut vcn = 0u64;
    let mut extents = Vec::with_capacity(runs.len());
    for run in runs.iter().filter(|r| r.clusters > 0) {
        extents.push(MftExtent {
            vcn,
            lcn: run.lcn,
            clusters: run.clusters,
        });
        vcn = vcn.saturating_add(run.clusters);
    }
    extents
}

/// Parses an `NTFS_FILE_RECORD_OUTPUT_BUFFER`: returns the record number that came back
/// (low 48 bits of the reference; the sequence number is ignored) and the record bytes.
pub(crate) fn file_record_bytes(buf: &[u8], record_size: u64) -> Result<(u64, &[u8]), ScanError> {
    let corrupt = || layout_error("corrupt FSCTL_GET_NTFS_FILE_RECORD output");
    let reference = read_i64(buf, 0).ok_or_else(corrupt)?;
    let rec_no = (reference as u64) & 0x0000_FFFF_FFFF_FFFF;
    let len = u64::from(read_u32(buf, 8).ok_or_else(corrupt)?);
    if len != record_size {
        return Err(corrupt());
    }
    let len = usize::try_from(len).map_err(|_| corrupt())?;
    let bytes = buf.get(FR_HEADER..FR_HEADER + len).ok_or_else(corrupt)?;
    Ok((rec_no, bytes))
}

/// One read from the volume into a block buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Segment {
    /// Byte offset on the volume.
    pub volume_offset: u64,
    /// Byte offset inside the block.
    pub buf_offset: usize,
    pub len: usize,
}

/// One block of the MFT: `len` bytes starting at byte `mft_offset` of the MFT. Bytes not
/// covered by a segment (unallocated parts) are zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BlockPlan {
    pub mft_offset: u64,
    pub len: usize,
    /// Sorted by `buf_offset`, non-overlapping.
    pub segments: Vec<Segment>,
}

/// Block size: about 8 MiB, a whole number of records (and of clusters when that is cheap).
pub(crate) fn block_size(geo: &Geometry) -> u64 {
    // lcm(record, cluster); just whole records if that is larger than the target.
    let lcm = geo
        .record
        .checked_mul(geo.cluster / gcd(geo.record, geo.cluster))
        .filter(|&unit| unit <= BLOCK_TARGET);
    let unit = lcm.unwrap_or(geo.record);
    (BLOCK_TARGET / unit).max(1) * unit
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Splits the MFT into blocks of `block` bytes and maps each onto volume reads.
///
/// Reads stop at `mft_valid_len` (rounded down to whole records) or at the end of the last
/// extent, whichever comes first. Every offset and length is a multiple of the sector size.
pub(crate) fn plan_blocks(
    extents: &[MftExtent],
    geo: &Geometry,
    block: u64,
) -> Result<Vec<BlockPlan>, ScanError> {
    if block == 0 || !block.is_multiple_of(geo.record) || usize::try_from(block).is_err() {
        return Err(layout_error(format!("bad block size {block}")));
    }
    let runs = byte_runs(extents, geo)?;
    let mapped_end = runs.last().map_or(0, |r| r.mft_end);
    // The MFT's allocation always covers its valid data, so extents ending earlier mean the
    // extent list is incomplete: refuse rather than silently miss records.
    if mapped_end < geo.mft_valid_len {
        return Err(layout_error(format!(
            "$MFT extents cover {mapped_end} bytes but {} bytes are in use",
            geo.mft_valid_len
        )));
    }
    let limit = geo.mft_valid_len - geo.mft_valid_len % geo.record;
    if limit == 0 {
        return Err(layout_error("no MFT data to read"));
    }

    let mut plans = Vec::new();
    let mut first_run = 0;
    let mut offset = 0u64;
    while offset < limit {
        let end = offset.saturating_add(block).min(limit);
        // Runs are sorted and do not overlap: skip those that end before this block.
        while first_run < runs.len() && runs[first_run].mft_end <= offset {
            first_run += 1;
        }
        let mut segments = Vec::new();
        for run in runs[first_run..].iter().take_while(|r| r.mft_start < end) {
            let start = run.mft_start.max(offset);
            let stop = run.mft_end.min(end);
            segments.push(Segment {
                volume_offset: run.volume_start + (start - run.mft_start),
                buf_offset: to_usize(start - offset)?,
                len: to_usize(stop - start)?,
            });
        }
        plans.push(BlockPlan {
            mft_offset: offset,
            len: to_usize(end - offset)?,
            segments,
        });
        offset = end;
    }
    Ok(plans)
}

/// An extent in bytes: MFT bytes `mft_start..mft_end` live on the volume at `volume_start`.
struct ByteRun {
    mft_start: u64,
    mft_end: u64,
    volume_start: u64,
}

/// Converts extents to byte ranges with checked math, sorted by MFT offset. Zero-length
/// extents are dropped; extents past the volume end or overlapping each other are errors.
fn byte_runs(extents: &[MftExtent], geo: &Geometry) -> Result<Vec<ByteRun>, ScanError> {
    let overflow = || layout_error("$MFT extent offsets overflow");
    let mut sorted: Vec<MftExtent> = extents.iter().copied().filter(|e| e.clusters > 0).collect();
    sorted.sort_unstable_by_key(|e| e.vcn);

    let mut runs: Vec<ByteRun> = Vec::with_capacity(sorted.len());
    for e in sorted {
        let lcn_end = e.lcn.checked_add(e.clusters).ok_or_else(overflow)?;
        if lcn_end > geo.total_clusters {
            return Err(layout_error(format!(
                "$MFT extent at cluster {} runs past the end of the volume",
                e.lcn
            )));
        }
        let mft_start = e.vcn.checked_mul(geo.cluster).ok_or_else(overflow)?;
        let len = e.clusters.checked_mul(geo.cluster).ok_or_else(overflow)?;
        let mft_end = mft_start.checked_add(len).ok_or_else(overflow)?;
        let volume_start = e.lcn.checked_mul(geo.cluster).ok_or_else(overflow)?;
        volume_start.checked_add(len).ok_or_else(overflow)?;
        if runs.last().is_some_and(|prev| prev.mft_end > mft_start) {
            return Err(layout_error("$MFT extents overlap"));
        }
        runs.push(ByteRun {
            mft_start,
            mft_end,
            volume_start,
        });
    }
    Ok(runs)
}

fn to_usize(n: u64) -> Result<usize, ScanError> {
    usize::try_from(n).map_err(|_| layout_error("block too large"))
}

fn read_u32(buf: &[u8], off: usize) -> Option<u32> {
    let bytes = buf.get(off..off.checked_add(4)?)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn read_i64(buf: &[u8], off: usize) -> Option<i64> {
    let bytes = buf.get(off..off.checked_add(8)?)?;
    Some(i64::from_le_bytes(bytes.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(sector: u32, cluster: u32, record: u32) -> VolumeInfo {
        VolumeInfo {
            serial: 1,
            bytes_per_sector: sector,
            bytes_per_cluster: cluster,
            bytes_per_record: record,
            mft_valid_data_length: 1 << 20,
            mft_start_lcn: 4,
            total_clusters: 1 << 20,
        }
    }

    fn geo(cluster: u64, record: u64, valid: u64) -> Geometry {
        Geometry {
            sector: 512,
            cluster,
            record,
            mft_valid_len: valid,
            total_clusters: 1 << 30,
        }
    }

    #[test]
    fn geometry_accepts_common_layouts() {
        let g = Geometry::new(&info(512, 4096, 1024)).unwrap();
        assert_eq!((g.sector, g.cluster, g.record), (512, 4096, 1024));
        assert_eq!((g.mft_valid_len, g.total_clusters), (1 << 20, 1 << 20));
        assert!(Geometry::new(&info(4096, 4096, 4096)).is_ok());
        assert!(Geometry::new(&info(512, 512, 1024)).is_ok());
        assert_eq!(
            Geometry::new(&info(512, 4096, 1024)).unwrap().sector_u32(),
            512
        );
    }

    #[test]
    fn geometry_rejects_bad_values() {
        for (s, c, r) in [
            (0, 4096, 1024),        // no sector size
            (512, 0, 1024),         // no cluster size
            (512, 4096, 0),         // no record size
            (512, 4096, 1000),      // record not a whole number of sectors
            (512, 4096, 128 << 10), // record larger than 64 KiB
            (4096, 6144, 4096),     // cluster not a whole number of sectors
            (500, 4000, 1000),      // sector not a power of two
        ] {
            let result = Geometry::new(&info(s, c, r));
            assert!(
                matches!(result, Err(ScanError::BadLayout(_))),
                "({s}, {c}, {r}) -> {result:?}"
            );
        }
        let mut no_clusters = info(512, 4096, 1024);
        no_clusters.total_clusters = 0;
        assert!(Geometry::new(&no_clusters).is_err());
    }

    fn rp_buffer(start_vcn: i64, pairs: &[(i64, i64)]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&(pairs.len() as u32).to_le_bytes());
        b.extend_from_slice(&[0; 4]);
        b.extend_from_slice(&start_vcn.to_le_bytes());
        for (next_vcn, lcn) in pairs {
            b.extend_from_slice(&next_vcn.to_le_bytes());
            b.extend_from_slice(&lcn.to_le_bytes());
        }
        b
    }

    #[test]
    fn retrieval_pointers_become_extents() {
        let buf = rp_buffer(0, &[(16, 1000), (20, -1), (36, 5000)]);
        let (extents, next) = parse_retrieval_pointers(&buf).unwrap();
        assert_eq!(
            extents,
            vec![
                MftExtent {
                    vcn: 0,
                    lcn: 1000,
                    clusters: 16
                },
                MftExtent {
                    vcn: 20,
                    lcn: 5000,
                    clusters: 16
                },
            ]
        );
        assert_eq!(next, 36);
    }

    #[test]
    fn retrieval_pointers_continue_from_starting_vcn() {
        let buf = rp_buffer(36, &[(40, 7000)]);
        let (extents, next) = parse_retrieval_pointers(&buf).unwrap();
        assert_eq!(
            extents,
            vec![MftExtent {
                vcn: 36,
                lcn: 7000,
                clusters: 4
            }]
        );
        assert_eq!(next, 40);
    }

    #[test]
    fn retrieval_pointers_reject_corrupt_buffers() {
        // Too short for the header.
        assert!(parse_retrieval_pointers(&[0; 8]).is_err());
        // Claims two pairs but holds one.
        let mut buf = rp_buffer(0, &[(16, 1000)]);
        buf[0] = 2;
        assert!(parse_retrieval_pointers(&buf).is_err());
        // VCN going backwards.
        assert!(parse_retrieval_pointers(&rp_buffer(10, &[(5, 1000)])).is_err());
        // Negative starting VCN and an LCN below -1.
        assert!(parse_retrieval_pointers(&rp_buffer(-4, &[(5, 1000)])).is_err());
        assert!(parse_retrieval_pointers(&rp_buffer(0, &[(5, -7)])).is_err());
    }

    #[test]
    fn runs_become_contiguous_extents() {
        let runs = [
            Extent {
                lcn: 100,
                clusters: 8,
            },
            Extent {
                lcn: 900,
                clusters: 0,
            },
            Extent {
                lcn: 500,
                clusters: 4,
            },
        ];
        assert_eq!(
            extents_from_runs(&runs),
            vec![
                MftExtent {
                    vcn: 0,
                    lcn: 100,
                    clusters: 8
                },
                MftExtent {
                    vcn: 8,
                    lcn: 500,
                    clusters: 4
                },
            ]
        );
    }

    #[test]
    fn file_record_output_is_validated() {
        let mut buf = vec![0u8; FR_HEADER + 1024];
        buf[8..12].copy_from_slice(&1024u32.to_le_bytes());
        buf[FR_HEADER] = b'F';
        let (rec_no, rec) = file_record_bytes(&buf, 1024).unwrap();
        assert_eq!((rec_no, rec.len(), rec[0]), (0, 1024, b'F'));

        // Another record number, with a sequence number in the top 16 bits.
        let mut other = buf.clone();
        other[0..8].copy_from_slice(&0x0003_0000_0000_0007u64.to_le_bytes());
        assert_eq!(file_record_bytes(&other, 1024).unwrap().0, 7);
        // Wrong length, or a buffer shorter than the claimed length.
        assert!(file_record_bytes(&buf, 4096).is_err());
        assert!(file_record_bytes(&buf[..FR_HEADER + 100], 1024).is_err());
    }

    #[test]
    fn block_size_is_whole_records_and_clusters() {
        assert_eq!(block_size(&geo(4096, 1024, 0)), 8 << 20);
        let odd = geo(3 * 4096, 4096, 0);
        let b = block_size(&odd);
        assert!(b <= 8 << 20 && b.is_multiple_of(3 * 4096), "{b}");
        // Huge clusters: whole records are enough.
        let big = Geometry {
            cluster: 2 << 30,
            ..geo(4096, 4096, 0)
        };
        assert_eq!(block_size(&big), 8 << 20);
    }

    #[test]
    fn single_extent_is_split_into_blocks() {
        // 10 records of 1 KiB, block of 4 KiB, MFT at cluster 100 (4 KiB clusters).
        let g = geo(4096, 1024, 10 * 1024);
        let extents = [MftExtent {
            vcn: 0,
            lcn: 100,
            clusters: 4,
        }];
        let plan = plan_blocks(&extents, &g, 4096).unwrap();
        assert_eq!(plan.len(), 3);
        assert_eq!((plan[0].mft_offset, plan[0].len), (0, 4096));
        assert_eq!(
            plan[0].segments,
            vec![Segment {
                volume_offset: 409_600,
                buf_offset: 0,
                len: 4096
            }]
        );
        assert_eq!((plan[2].mft_offset, plan[2].len), (8192, 2048));
        assert_eq!(plan[2].segments[0].volume_offset, 409_600 + 8192);
    }

    #[test]
    fn block_spanning_two_extents_gets_two_segments() {
        // Records of 1 KiB, clusters of 512 bytes: a record may straddle two extents.
        let g = geo(512, 1024, 4096);
        let extents = [
            MftExtent {
                vcn: 0,
                lcn: 10,
                clusters: 3,
            },
            MftExtent {
                vcn: 3,
                lcn: 50,
                clusters: 5,
            },
        ];
        let plan = plan_blocks(&extents, &g, 4096).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(
            plan[0].segments,
            vec![
                Segment {
                    volume_offset: 5120,
                    buf_offset: 0,
                    len: 1536
                },
                Segment {
                    volume_offset: 25_600,
                    buf_offset: 1536,
                    len: 2560
                },
            ]
        );
    }

    #[test]
    fn unallocated_vcns_leave_gaps_and_order_does_not_matter() {
        let g = geo(1024, 1024, 8192);
        let extents = [
            MftExtent {
                vcn: 6,
                lcn: 90,
                clusters: 2,
            },
            MftExtent {
                vcn: 0,
                lcn: 10,
                clusters: 2,
            },
        ];
        let plan = plan_blocks(&extents, &g, 8192).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(
            plan[0].segments,
            vec![
                Segment {
                    volume_offset: 10 * 1024,
                    buf_offset: 0,
                    len: 2048
                },
                Segment {
                    volume_offset: 90 * 1024,
                    buf_offset: 6144,
                    len: 2048
                },
            ]
        );
    }

    #[test]
    fn reads_stop_at_valid_length() {
        // Valid length not a whole record: rounded down.
        let g = geo(4096, 1024, 2500);
        let extents = [MftExtent {
            vcn: 0,
            lcn: 1,
            clusters: 4,
        }];
        let plan = plan_blocks(&extents, &g, 8192).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].len, 2048);
        assert_eq!(plan[0].segments[0].len, 2048);

        // Valid length exactly at the end of the extents.
        let g = geo(4096, 1024, 4 * 4096);
        let plan = plan_blocks(&extents, &g, 8192).unwrap();
        let total: usize = plan.iter().map(|b| b.len).sum();
        assert_eq!(total, 4 * 4096);
    }

    #[test]
    fn extents_shorter_than_valid_data_are_rejected() {
        // Runs stored in extension records were not seen: refuse to scan a partial MFT.
        let g = geo(4096, 1024, 1 << 20);
        let extents = [MftExtent {
            vcn: 0,
            lcn: 1,
            clusters: 4,
        }];
        assert!(matches!(
            plan_blocks(&extents, &g, 8192),
            Err(ScanError::BadLayout(_))
        ));
    }

    #[test]
    fn zero_cluster_extents_are_skipped() {
        let g = geo(4096, 1024, 4096);
        let extents = [
            MftExtent {
                vcn: 0,
                lcn: 7,
                clusters: 0,
            },
            MftExtent {
                vcn: 0,
                lcn: 1,
                clusters: 1,
            },
        ];
        let plan = plan_blocks(&extents, &g, 8192).unwrap();
        assert_eq!(plan[0].segments.len(), 1);
        assert_eq!(plan[0].segments[0].volume_offset, 4096);
    }

    #[test]
    fn bad_extents_are_rejected() {
        let g = Geometry {
            total_clusters: 1000,
            ..geo(4096, 1024, 1 << 20)
        };
        // Past the end of the volume.
        let past_end = [MftExtent {
            vcn: 0,
            lcn: 999,
            clusters: 2,
        }];
        assert!(plan_blocks(&past_end, &g, 8192).is_err());
        // Overflowing byte offsets (on a volume that claims to be that large).
        let huge = Geometry {
            total_clusters: u64::MAX,
            ..g
        };
        let overflow = [MftExtent {
            vcn: 0,
            lcn: u64::MAX / 2,
            clusters: 1,
        }];
        assert!(matches!(
            plan_blocks(&overflow, &huge, 8192),
            Err(ScanError::BadLayout(m)) if m.contains("overflow")
        ));
        let vcn_overflow = [MftExtent {
            vcn: u64::MAX / 2,
            lcn: 1,
            clusters: 1,
        }];
        assert!(plan_blocks(&vcn_overflow, &huge, 8192).is_err());
        // Overlapping VCN ranges.
        let overlap = [
            MftExtent {
                vcn: 0,
                lcn: 1,
                clusters: 4,
            },
            MftExtent {
                vcn: 2,
                lcn: 10,
                clusters: 4,
            },
        ];
        assert!(plan_blocks(&overlap, &g, 8192).is_err());
        // Nothing to read.
        assert!(plan_blocks(&[], &g, 8192).is_err());
    }
}
