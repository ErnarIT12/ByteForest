//! [`MftScanner`]: reads the whole Master File Table from the raw volume and parses its
//! records in parallel.
//!
//! This module holds the Windows I/O and the pipeline. The pure parts live in
//! [`crate::mft_layout`] (where to read) and [`crate::mft_merge`] (records to entries).
#![allow(unsafe_code)]

use std::fs::{File, OpenOptions};
use std::io;
use std::mem::size_of;
use std::os::windows::fs::{FileExt, OpenOptionsExt};
use std::os::windows::io::AsRawHandle;
use std::ptr;
use std::time::Instant;

use bf_core::{NodeFlags, Tree, TreeBuilder};
use bf_ntfs::{FileRecord, NtfsError, VolumeInfo, apply_fixups, parse_record, parse_volume_data};
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use rayon::prelude::*;
use windows::Win32::Foundation::{ERROR_HANDLE_EOF, ERROR_MORE_DATA, HANDLE};
use windows::Win32::Storage::FileSystem::{
    FILE_FLAG_NO_BUFFERING, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::{
    FSCTL_GET_NTFS_FILE_RECORD, FSCTL_GET_NTFS_VOLUME_DATA, FSCTL_GET_RETRIEVAL_POINTERS,
    NTFS_FILE_RECORD_INPUT_BUFFER, STARTING_VCN_INPUT_BUFFER,
};

use crate::mft_layout::{
    BlockPlan, Geometry, MftExtent, block_size, extents_from_runs, file_record_bytes,
    parse_retrieval_pointers, plan_blocks,
};
use crate::mft_merge::{ROOT_RECORD, Slot, merge_records, parse_slot, root_name};
use crate::progress::{Phase, ProgressReporter, ScanProgress};
use crate::{CancelToken, ProgressFn, ScanError, ScanMethod, ScanRoot, ScanStats, Scanner};

/// Blocks in flight between the reader thread and the parser.
const CHANNEL_DEPTH: usize = 4;
/// Minimum buffer alignment for unbuffered volume reads (raised to the sector size).
const MIN_ALIGN: usize = 4096;
/// Output buffer for `FSCTL_GET_NTFS_VOLUME_DATA` (the base struct plus extended data).
const VOLUME_DATA_LEN: usize = 512;
/// Output buffer for `FSCTL_GET_RETRIEVAL_POINTERS`, in `u64` words (64 KiB).
const RP_WORDS: usize = 8192;
/// Upper bound on `FSCTL_GET_RETRIEVAL_POINTERS` calls (each returns ~4000 extents).
const MAX_RP_CALLS: usize = 4096;
/// Header of `NTFS_FILE_RECORD_OUTPUT_BUFFER` before the record bytes.
const FILE_RECORD_HEADER: usize = 12;

/// Scanner that reads the NTFS Master File Table directly.
///
/// Needs administrator rights and an NTFS volume; only [`ScanRoot::Volume`] is supported.
/// One thread reads the MFT in 8 MiB blocks while rayon parses the records of earlier blocks.
/// Extension records are merged into their base record, and every extra hard link becomes a
/// node of size 0 flagged `HARDLINK`, so each file's size is counted once.
pub struct MftScanner;

impl Scanner for MftScanner {
    fn scan(
        &self,
        root: &ScanRoot,
        progress: ProgressFn<'_>,
        cancel: &CancelToken,
    ) -> Result<(Tree, ScanStats), ScanError> {
        match root {
            ScanRoot::Volume(letter) => scan_volume(*letter, progress, cancel),
            ScanRoot::Folder(_) => Err(ScanError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the MFT scanner reads whole volumes, not folders",
            ))),
        }
    }
}

fn scan_volume(
    letter: char,
    progress: ProgressFn<'_>,
    cancel: &CancelToken,
) -> Result<(Tree, ScanStats), ScanError> {
    let start = Instant::now();
    if cancel.is_cancelled() {
        return Err(ScanError::Cancelled);
    }
    if !letter.is_ascii_alphabetic() {
        return Err(ScanError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid drive letter {letter:?}"),
        )));
    }
    let letter = letter.to_ascii_uppercase();

    let volume = open_volume(letter)?;
    let geo = Geometry::new(&volume_data(&volume)?)?;
    let extents = mft_extents(letter, &volume, &geo)?;
    let plan = plan_blocks(&extents, &geo, block_size(&geo))?;

    let reporter = ProgressReporter::new(progress);
    let (records, skipped) = read_and_parse(&volume, &plan, &geo, &reporter, cancel)?;
    drop(volume);
    reporter.finish(Phase::Parsing);

    let mut builder = TreeBuilder::new(ROOT_RECORD, &root_name(letter));
    let mut counts = ScanProgress {
        phase: Phase::Building,
        ..ScanProgress::default()
    };
    let merged = merge_records(records, |entry| {
        if entry.flags.contains(NodeFlags::DIR) {
            counts.dirs += 1;
        } else {
            counts.files += 1;
            counts.bytes = counts.bytes.saturating_add(entry.size);
        }
        builder.push(entry);
    });
    if cancel.is_cancelled() {
        return Err(ScanError::Cancelled);
    }
    progress(counts);
    let tree = builder.finish();

    let total = tree.total(tree.root());
    let stats = ScanStats {
        files: total.files,
        dirs: total.dirs,
        bytes: total.size,
        elapsed_ms: start.elapsed().as_millis() as u64,
        method: ScanMethod::Mft,
        fallback_reason: None,
        skipped_records: skipped.saturating_add(merged.dropped_extensions),
        errors: 0,
    };
    Ok((tree, stats))
}

/// The raw Win32 handle of an open file, for `DeviceIoControl`.
fn raw(file: &File) -> HANDLE {
    HANDLE(file.as_raw_handle())
}

fn open_error(err: io::Error) -> ScanError {
    if err.kind() == io::ErrorKind::PermissionDenied {
        ScanError::AccessDenied
    } else {
        ScanError::Io(err)
    }
}

fn win_error(err: windows::core::Error) -> ScanError {
    ScanError::Io(io::Error::from(err))
}

/// Opens `\\.\X:` for unbuffered reading. `File` closes the handle on drop.
fn open_volume(letter: char) -> Result<File, ScanError> {
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0)
        .custom_flags(FILE_FLAG_NO_BUFFERING.0)
        .open(format!(r"\\.\{letter}:"))
        .map_err(open_error)
}

/// Queries `FSCTL_GET_NTFS_VOLUME_DATA`; fails with `NotNtfs` on other file systems.
fn volume_data(volume: &File) -> Result<VolumeInfo, ScanError> {
    let mut out = [0u8; VOLUME_DATA_LEN];
    let mut returned = 0u32;
    // SAFETY: the handle is owned by `volume`, which outlives the call. No input buffer is
    // passed; the output pointer and length describe `out`, and `returned` is a valid
    // out-pointer. The call is synchronous (no OVERLAPPED).
    let result = unsafe {
        DeviceIoControl(
            raw(volume),
            FSCTL_GET_NTFS_VOLUME_DATA,
            None,
            0,
            Some(out.as_mut_ptr().cast()),
            out.len() as u32,
            Some(&mut returned),
            None,
        )
    };
    match result {
        Ok(()) => {}
        Err(e) if e.code() == ERROR_MORE_DATA.to_hresult() => {}
        Err(_) => return Err(ScanError::NotNtfs),
    }
    let len = usize::try_from(returned).map_or(out.len(), |n| n.min(out.len()));
    Ok(parse_volume_data(&out[..len])?)
}

/// Extents of `$MFT`: from `FSCTL_GET_RETRIEVAL_POINTERS`, else from the data runs of
/// record 0 (which may miss runs stored in extension records of a very fragmented MFT).
fn mft_extents(letter: char, volume: &File, geo: &Geometry) -> Result<Vec<MftExtent>, ScanError> {
    let first = match retrieval_pointers(letter) {
        Ok(extents) if !extents.is_empty() => return Ok(extents),
        Ok(_) => "no extents returned".to_string(),
        Err(e) => e.to_string(),
    };
    record_zero_extents(volume, geo).map_err(|e| {
        ScanError::BadLayout(format!(
            "cannot locate $MFT (retrieval pointers: {first}; record 0: {e})"
        ))
    })
}

/// Copies the first `len` bytes of a `u64` buffer into a byte vector (little-endian).
fn words_to_bytes(words: &[u64], len: usize) -> Vec<u8> {
    words
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .take(len)
        .collect()
}

/// Asks NTFS where the clusters of `X:\$MFT` are, looping while it reports more data.
fn retrieval_pointers(letter: char) -> Result<Vec<MftExtent>, ScanError> {
    let mft = OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES.0)
        .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0)
        .open(format!(r"{letter}:\$MFT"))
        .map_err(open_error)?;

    // `u64` words keep the output 8-byte aligned, as the structure requires.
    let mut out = vec![0u64; RP_WORDS];
    let out_len = RP_WORDS * size_of::<u64>();
    let mut extents = Vec::new();
    let mut next_vcn = 0u64;
    for _ in 0..MAX_RP_CALLS {
        let input = STARTING_VCN_INPUT_BUFFER {
            StartingVcn: i64::try_from(next_vcn)
                .map_err(|_| ScanError::BadLayout("$MFT VCN out of range".into()))?,
        };
        let mut returned = 0u32;
        // SAFETY: the handle is owned by `mft`, which outlives the call. The input pointer and
        // size describe `input`; the output pointer and size describe `out` (8-byte aligned);
        // `returned` is a valid out-pointer. The call is synchronous (no OVERLAPPED).
        let result = unsafe {
            DeviceIoControl(
                raw(&mft),
                FSCTL_GET_RETRIEVAL_POINTERS,
                Some(ptr::from_ref(&input).cast()),
                size_of::<STARTING_VCN_INPUT_BUFFER>() as u32,
                Some(out.as_mut_ptr().cast()),
                out_len as u32,
                Some(&mut returned),
                None,
            )
        };
        let more = match result {
            Ok(()) => false,
            Err(e) if e.code() == ERROR_MORE_DATA.to_hresult() => true,
            // The starting VCN is past the end of the file: nothing more to map.
            Err(e) if e.code() == ERROR_HANDLE_EOF.to_hresult() => return Ok(extents),
            Err(e) => return Err(win_error(e)),
        };
        // Some drivers leave `returned` at 0 on ERROR_MORE_DATA; the parser checks the
        // extent count against the buffer length either way.
        let len = match usize::try_from(returned) {
            Ok(0) | Err(_) => out_len,
            Ok(n) => n.min(out_len),
        };
        let (mut batch, end_vcn) = parse_retrieval_pointers(&words_to_bytes(&out, len))?;
        extents.append(&mut batch);
        if !more {
            return Ok(extents);
        }
        if end_vcn <= next_vcn {
            return Err(ScanError::BadLayout(
                "retrieval pointers made no progress".into(),
            ));
        }
        next_vcn = end_vcn;
    }
    Err(ScanError::BadLayout("too many $MFT extents".into()))
}

/// Reads record 0 with `FSCTL_GET_NTFS_FILE_RECORD` and decodes the runs of its `$DATA`.
fn record_zero_extents(volume: &File, geo: &Geometry) -> Result<Vec<MftExtent>, ScanError> {
    let record_len =
        usize::try_from(geo.record).map_err(|_| ScanError::BadLayout("record too large".into()))?;
    let words = (FILE_RECORD_HEADER + record_len).div_ceil(size_of::<u64>());
    let out_len = words * size_of::<u64>();
    let mut out = vec![0u64; words];
    let input = NTFS_FILE_RECORD_INPUT_BUFFER {
        FileReferenceNumber: 0,
    };
    let mut returned = 0u32;
    // SAFETY: the handle is owned by `volume`, which outlives the call. The input pointer and
    // size describe `input`; the output pointer and size describe `out`; `returned` is a valid
    // out-pointer. The call is synchronous (no OVERLAPPED).
    unsafe {
        DeviceIoControl(
            raw(volume),
            FSCTL_GET_NTFS_FILE_RECORD,
            Some(ptr::from_ref(&input).cast()),
            size_of::<NTFS_FILE_RECORD_INPUT_BUFFER>() as u32,
            Some(out.as_mut_ptr().cast()),
            out_len as u32,
            Some(&mut returned),
            None,
        )
    }
    .map_err(win_error)?;

    let len = usize::try_from(returned).map_or(out_len, |n| n.min(out_len));
    let bytes = words_to_bytes(&out, len);
    let mut record = file_record_bytes(&bytes, geo.record)?.to_vec();
    match apply_fixups(&mut record, geo.sector_u32()) {
        // A mismatch means the record came back with its fixups already applied.
        Ok(()) | Err(NtfsError::FixupMismatch) => {}
        Err(e) => return Err(e.into()),
    }
    let parsed = parse_record(&record, 0)?
        .ok_or_else(|| ScanError::BadLayout("$MFT record is not in use".into()))?;
    let runs = parsed
        .mft_runs
        .ok_or_else(|| ScanError::BadLayout("$MFT record has no data runs".into()))?;
    Ok(extents_from_runs(&runs))
}

/// One block read from the MFT; the data is `buf[start..start + len]`.
struct Block {
    buf: Vec<u8>,
    start: usize,
    len: usize,
    /// Byte offset of the block inside the MFT.
    mft_offset: u64,
}

/// Runs the reader thread and parses its blocks as they arrive.
///
/// Returns the in-use records and the number of records that failed to parse.
fn read_and_parse(
    volume: &File,
    plan: &[BlockPlan],
    geo: &Geometry,
    reporter: &ProgressReporter<'_>,
    cancel: &CancelToken,
) -> Result<(Vec<FileRecord>, u64), ScanError> {
    let align = MIN_ALIGN.max(usize::try_from(geo.sector).unwrap_or(MIN_ALIGN));
    let capacity = plan.iter().map(|b| b.len).max().unwrap_or(0) + align;
    let (full_tx, full_rx) = bounded::<Block>(CHANNEL_DEPTH);
    // Parsed blocks go back to the reader so their buffers are reused.
    let (free_tx, free_rx) = unbounded::<Vec<u8>>();

    std::thread::scope(|scope| {
        let reader = scope.spawn(move || {
            let reader = Reader {
                volume,
                align,
                capacity,
                full: full_tx,
                free: free_rx,
                reporter,
                cancel,
            };
            reader.run(plan)
        });

        let mut records = Vec::new();
        let mut skipped = 0u64;
        for mut block in full_rx.iter() {
            if cancel.is_cancelled() {
                break;
            }
            let (mut parsed, bad) = parse_block(&mut block, geo);
            records.append(&mut parsed);
            skipped += bad;
            // The reader may already be done; then the buffer is simply dropped.
            let _ = free_tx.send(block.buf);
        }
        // Unblocks a reader waiting on a full channel after an early stop.
        drop(full_rx);

        reader
            .join()
            .unwrap_or_else(|_| Err(ScanError::Io(io::Error::other("MFT reader panicked"))))?;
        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        Ok((records, skipped))
    })
}

/// The reader thread's state: reads planned blocks and sends them to the parser.
struct Reader<'a> {
    volume: &'a File,
    align: usize,
    capacity: usize,
    full: Sender<Block>,
    free: Receiver<Vec<u8>>,
    reporter: &'a ProgressReporter<'a>,
    cancel: &'a CancelToken,
}

impl Reader<'_> {
    fn run(&self, plan: &[BlockPlan]) -> Result<(), ScanError> {
        for planned in plan {
            if self.cancel.is_cancelled() {
                return Err(ScanError::Cancelled);
            }
            let mut buf = self
                .free
                .try_recv()
                .unwrap_or_else(|_| vec![0u8; self.capacity]);
            let start = buf.as_ptr().align_offset(self.align);
            let data = start
                .checked_add(planned.len)
                .and_then(|end| buf.get_mut(start..end))
                .ok_or_else(|| io::Error::other("cannot align the read buffer"))?;
            fill_block(self.volume, planned, data)?;
            self.reporter.tick(0, 0, planned.len as u64, Phase::Reading);

            let block = Block {
                buf,
                start,
                len: planned.len,
                mft_offset: planned.mft_offset,
            };
            if self.full.send(block).is_err() {
                return Ok(()); // the parser stopped early
            }
        }
        Ok(())
    }
}

/// Reads the segments of one block into `data`; bytes no segment covers are zeroed.
fn fill_block(volume: &File, planned: &BlockPlan, data: &mut [u8]) -> io::Result<()> {
    let bad_plan = || io::Error::other("inconsistent MFT read plan");
    let mut pos = 0;
    for seg in &planned.segments {
        let end = seg.buf_offset.checked_add(seg.len).ok_or_else(bad_plan)?;
        data.get_mut(pos..seg.buf_offset)
            .ok_or_else(bad_plan)?
            .fill(0);
        let target = data.get_mut(seg.buf_offset..end).ok_or_else(bad_plan)?;
        read_exact_at(volume, target, seg.volume_offset)?;
        pos = end;
    }
    data.get_mut(pos..).ok_or_else(bad_plan)?.fill(0);
    Ok(())
}

/// Reads exactly `buf.len()` bytes at `offset`, retrying after short reads.
fn read_exact_at(file: &File, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
    while !buf.is_empty() {
        let n = file.seek_read(buf, offset)?;
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        buf = &mut buf[n..];
        offset += n as u64;
    }
    Ok(())
}

/// Parses every record slot of a block in parallel.
///
/// The record number comes from the block's position in the MFT, not from read order.
fn parse_block(block: &mut Block, geo: &Geometry) -> (Vec<FileRecord>, u64) {
    let (Ok(record_len), Some(end)) = (
        usize::try_from(geo.record),
        block.start.checked_add(block.len),
    ) else {
        return (Vec::new(), 0);
    };
    let Some(data) = block.buf.get_mut(block.start..end) else {
        return (Vec::new(), 0);
    };
    let first = block.mft_offset / geo.record;
    let sector = geo.sector_u32();

    data.par_chunks_mut(record_len)
        .enumerate()
        .fold(
            || (Vec::new(), 0u64),
            |(mut records, mut bad), (i, slot)| {
                match parse_slot(slot, first + i as u64, sector) {
                    Slot::Used(record) => records.push(record),
                    Slot::Bad => bad += 1,
                    Slot::Free => {}
                }
                (records, bad)
            },
        )
        .reduce(
            || (Vec::new(), 0),
            |(mut a, bad_a), (mut b, bad_b)| {
                a.append(&mut b);
                (a, bad_a + bad_b)
            },
        )
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::mft_merge::tests::raw_record;

    const RECORD: usize = 1024;

    /// A regular file that stands in for the volume: 64 clusters of 1 KiB.
    fn fake_volume(records: &[(u64, Vec<u8>)]) -> (tempfile::NamedTempFile, File) {
        let mut image = vec![0u8; 64 * 1024];
        for (offset, bytes) in records {
            let at = *offset as usize;
            image[at..at + bytes.len()].copy_from_slice(bytes);
        }
        let preferred = std::path::Path::new(r"D:\Temp");
        let mut tmp = if preferred.is_dir() {
            tempfile::NamedTempFile::new_in(preferred).unwrap()
        } else {
            tempfile::NamedTempFile::new().unwrap()
        };
        tmp.write_all(&image).unwrap();
        tmp.flush().unwrap();
        let file = File::open(tmp.path()).unwrap();
        (tmp, file)
    }

    fn geometry(valid: u64) -> Geometry {
        Geometry {
            sector: 512,
            cluster: 1024,
            record: RECORD as u64,
            mft_valid_len: valid,
            total_clusters: 64,
        }
    }

    #[test]
    fn pipeline_numbers_records_by_mft_position() {
        let mut torn = raw_record(RECORD, true);
        torn[RECORD - 2] = 0x77;
        // MFT records 0-1 at cluster 10, 2-3 unallocated, 4-5 at cluster 30.
        let (_tmp, volume) = fake_volume(&[
            (10 * 1024, raw_record(RECORD, true)),
            (11 * 1024, raw_record(RECORD, true)),
            (30 * 1024, torn),
            (31 * 1024, raw_record(RECORD, true)),
        ]);
        let extents = [
            MftExtent {
                vcn: 0,
                lcn: 10,
                clusters: 2,
            },
            MftExtent {
                vcn: 4,
                lcn: 30,
                clusters: 2,
            },
        ];
        let geo = geometry(6 * 1024);
        let plan = plan_blocks(&extents, &geo, 2048).unwrap();
        assert_eq!(plan.len(), 3);

        let reads = std::sync::Mutex::new(Vec::new());
        let cb = |p: ScanProgress| reads.lock().unwrap().push(p);
        let reporter = ProgressReporter::new(&cb);
        let (records, skipped) =
            read_and_parse(&volume, &plan, &geo, &reporter, &CancelToken::new()).unwrap();
        reporter.finish(Phase::Parsing);

        let mut numbers: Vec<u64> = records.iter().map(|r| r.rec_no).collect();
        numbers.sort_unstable();
        assert_eq!(numbers, vec![0, 1, 5]);
        assert_eq!(skipped, 1, "the torn record 4");
        let last = *reads.lock().unwrap().last().unwrap();
        assert_eq!((last.bytes, last.phase), (6 * 1024, Phase::Parsing));
    }

    #[test]
    fn pipeline_stops_when_cancelled() {
        let (_tmp, volume) = fake_volume(&[(0, raw_record(RECORD, true))]);
        let extents = [MftExtent {
            vcn: 0,
            lcn: 0,
            clusters: 32,
        }];
        let geo = geometry(32 * 1024);
        let plan = plan_blocks(&extents, &geo, 2048).unwrap();
        let cancel = CancelToken::new();
        cancel.cancel();
        let reporter = ProgressReporter::new(&|_| {});
        let result = read_and_parse(&volume, &plan, &geo, &reporter, &cancel);
        assert!(matches!(result, Err(ScanError::Cancelled)));
    }

    #[test]
    fn pipeline_reports_read_errors() {
        // The plan points past the end of the (short) file.
        let (_tmp, volume) = fake_volume(&[]);
        let extents = [MftExtent {
            vcn: 0,
            lcn: 100,
            clusters: 4,
        }];
        let geo = Geometry {
            total_clusters: 1000,
            ..geometry(4 * 1024)
        };
        let plan = plan_blocks(&extents, &geo, 2048).unwrap();
        let reporter = ProgressReporter::new(&|_| {});
        let result = read_and_parse(&volume, &plan, &geo, &reporter, &CancelToken::new());
        assert!(matches!(result, Err(ScanError::Io(_))), "{result:?}");
    }

    #[test]
    fn mft_scanner_rejects_bad_drive_letters() {
        let result = MftScanner.scan(&ScanRoot::Volume('1'), &|_| {}, &CancelToken::new());
        assert!(matches!(result, Err(ScanError::Io(_))));
    }

    #[test]
    fn mft_scanner_without_admin_fails_cleanly() {
        if crate::is_elevated() {
            return;
        }
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        let letter = drive.chars().next().unwrap_or('C');
        let result = MftScanner.scan(&ScanRoot::Volume(letter), &|_| {}, &CancelToken::new());
        assert!(matches!(result, Err(ScanError::AccessDenied)), "{result:?}");
    }
}
