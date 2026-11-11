//! [`MftScanner`]: reads the whole Master File Table from the raw volume and parses its
//! records in parallel.
//!
//! This module holds the Windows I/O and the pipeline. The pure parts live in
//! [`crate::mft_layout`] (where to read) and [`crate::mft_merge`] (records to entries); raw
//! volume reads live in [`crate::volume_io`]. Every error names the step that failed.
#![allow(unsafe_code)]

use std::fs::OpenOptions;
use std::io;
use std::mem::size_of;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::ptr;
use std::time::Instant;

use bf_core::{NodeFlags, Tree, TreeBuilder};
use bf_ntfs::{FileRecord, NtfsError, VolumeInfo, apply_fixups, parse_record, parse_volume_data};
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use rayon::prelude::*;
use windows::Win32::Foundation::{
    ERROR_HANDLE_EOF, ERROR_INVALID_FUNCTION, ERROR_MORE_DATA, HANDLE,
};
use windows::Win32::Storage::FileSystem::{
    FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::{
    FSCTL_GET_NTFS_FILE_RECORD, FSCTL_GET_NTFS_VOLUME_DATA, FSCTL_GET_RETRIEVAL_POINTERS,
    NTFS_FILE_RECORD_INPUT_BUFFER, STARTING_VCN_INPUT_BUFFER,
};

use crate::mft_fallback::{split_ranges, walk_down};
use crate::mft_layout::{
    BlockPlan, Geometry, MftExtent, block_size, extents_from_runs, file_record_bytes,
    parse_retrieval_pointers, plan_blocks,
};
use crate::mft_merge::{
    ROOT_RECORD, Slot, merge_records, parse_fetched_record, parse_slot, root_name,
};
use crate::progress::{Phase, ProgressReporter, ScanProgress};
use crate::volume_io::{Volume, context, win32_error};
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
/// Most threads used for record-by-record reading.
const MAX_RECORD_THREADS: usize = 16;
/// Record bytes fetched between two progress ticks in record-by-record reading.
const RECORD_PROGRESS_BYTES: u64 = 1 << 20;

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

/// Upper-case drive letter, or an `InvalidInput` error.
pub(crate) fn check_letter(letter: char) -> Result<char, ScanError> {
    if letter.is_ascii_alphabetic() {
        Ok(letter.to_ascii_uppercase())
    } else {
        Err(ScanError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid drive letter {letter:?}"),
        )))
    }
}

/// How [`scan_volume_with`] reads the MFT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadMode {
    /// Raw block reads, or record by record if those are not possible.
    Auto,
    /// Record by record with `FSCTL_GET_NTFS_FILE_RECORD` only (used by the probe).
    RecordsOnly,
}

fn scan_volume(
    letter: char,
    progress: ProgressFn<'_>,
    cancel: &CancelToken,
) -> Result<(Tree, ScanStats), ScanError> {
    scan_volume_with(letter, progress, cancel, ReadMode::Auto)
}

/// Scans drive `letter` through its MFT, reading it as `mode` says.
pub(crate) fn scan_volume_with(
    letter: char,
    progress: ProgressFn<'_>,
    cancel: &CancelToken,
    mode: ReadMode,
) -> Result<(Tree, ScanStats), ScanError> {
    let start = Instant::now();
    if cancel.is_cancelled() {
        return Err(ScanError::Cancelled);
    }
    let letter = check_letter(letter)?;

    let volume = open_volume(letter, true)?;
    let geo = Geometry::new(&volume_data(&volume)?)?;

    let reporter = ProgressReporter::new(progress);
    let by_records = || read_by_records(letter, &geo, &reporter, cancel);
    let (records, skipped, fallback_reason) = match mode {
        ReadMode::Auto => read_with_fallback(
            block_reader(letter, volume, &geo),
            |(volume, plan)| read_and_parse(&volume, &plan, &geo, &reporter, cancel),
            by_records,
        )?,
        ReadMode::RecordsOnly => {
            let (records, skipped) = by_records()?;
            (records, skipped, None)
        }
    };
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
        fallback_reason,
        skipped_records: skipped.saturating_add(merged.dropped_extensions),
        errors: 0,
    };
    Ok((tree, stats))
}

/// In-use records and the number of records that could not be read or parsed.
type ReadOutcome = (Vec<FileRecord>, u64);

/// Reads the MFT in raw blocks when `blocks` says that is possible, else record by record.
///
/// Raw reads can be blocked from the start (for example by an anti-cheat file-system
/// filter) or fail in the middle of the scan; either way the whole MFT is read again with
/// `read_records`, and the third value says why. A cancelled scan is never retried.
fn read_with_fallback<B>(
    blocks: Result<B, ScanError>,
    read_blocks: impl FnOnce(B) -> Result<ReadOutcome, ScanError>,
    read_records: impl FnOnce() -> Result<ReadOutcome, ScanError>,
) -> Result<(Vec<FileRecord>, u64, Option<String>), ScanError> {
    let reason = match blocks {
        Ok(blocks) => match read_blocks(blocks) {
            Ok((records, skipped)) => return Ok((records, skipped, None)),
            Err(ScanError::Cancelled) => return Err(ScanError::Cancelled),
            Err(err) => format!("raw MFT read failed mid-scan ({err}); read records one by one"),
        },
        Err(ScanError::Cancelled) => return Err(ScanError::Cancelled),
        Err(err) => format!("raw MFT reads blocked ({err}); read records one by one"),
    };
    let (records, skipped) = read_records()?;
    Ok((records, skipped, Some(reason)))
}

/// Locates the MFT extents, plans the block reads and checks that the first block can be
/// read. Returns the handle to read with and the plan, or why block reads are not possible.
pub(crate) fn block_reader(
    letter: char,
    volume: Volume,
    geo: &Geometry,
) -> Result<(Volume, Vec<BlockPlan>), ScanError> {
    let extents = mft_extents(letter, &volume, geo)?;
    let plan = plan_blocks(&extents, geo, block_size(geo))?;
    let volume = readable_volume(letter, volume, &plan, geo)?;
    Ok((volume, plan))
}

/// Opens `\\.\X:`; a permission error becomes [`ScanError::AccessDenied`].
pub(crate) fn open_volume(letter: char, unbuffered: bool) -> Result<Volume, ScanError> {
    Volume::open_drive(letter, unbuffered).map_err(|err| {
        if err.kind() == io::ErrorKind::PermissionDenied {
            ScanError::AccessDenied
        } else {
            ScanError::Io(err)
        }
    })
}

/// Makes sure the first planned block can be read. If the unbuffered handle fails, the
/// volume is reopened without `FILE_FLAG_NO_BUFFERING` and tried again; the handle that works
/// is returned. If both fail, the error describes both attempts.
fn readable_volume(
    letter: char,
    volume: Volume,
    plan: &[BlockPlan],
    geo: &Geometry,
) -> Result<Volume, ScanError> {
    let Some(first) = plan.first() else {
        return Ok(volume);
    };
    let unbuffered_err = match test_read(&volume, first, geo) {
        Ok(()) => return Ok(volume),
        Err(e) => e,
    };
    drop(volume);
    let buffered = open_volume(letter, false)?;
    match test_read(&buffered, first, geo) {
        Ok(()) => Ok(buffered),
        Err(e) => Err(ScanError::Io(io::Error::new(
            e.kind(),
            format!("{e}; the unbuffered attempt failed too: {unbuffered_err}"),
        ))),
    }
}

/// Reads one planned block into a scratch buffer.
pub(crate) fn test_read(volume: &Volume, block: &BlockPlan, geo: &Geometry) -> io::Result<()> {
    let align = read_alignment(geo);
    let mut buf = vec![0u8; block.len + align];
    let (_, data) = aligned(&mut buf, align, block.len)?;
    fill_block(volume, block, data)
}

/// Buffer alignment for unbuffered reads: at least 4 KiB and at least one sector.
pub(crate) fn read_alignment(geo: &Geometry) -> usize {
    MIN_ALIGN.max(usize::try_from(geo.sector).unwrap_or(MIN_ALIGN))
}

/// Returns the aligned window of `len` bytes inside `buf` and its start offset.
pub(crate) fn aligned(buf: &mut [u8], align: usize, len: usize) -> io::Result<(usize, &mut [u8])> {
    let start = buf.as_ptr().align_offset(align);
    let data = start
        .checked_add(len)
        .and_then(|end| buf.get_mut(start..end))
        .ok_or_else(|| io::Error::other(format!("cannot align a {len}-byte read buffer")))?;
    Ok((start, data))
}

/// Queries `FSCTL_GET_NTFS_VOLUME_DATA`. `ERROR_INVALID_FUNCTION` (another file system)
/// becomes [`ScanError::NotNtfs`]; any other failure is an I/O error naming the FSCTL.
pub(crate) fn volume_data(volume: &Volume) -> Result<VolumeInfo, ScanError> {
    let step = format!("FSCTL_GET_NTFS_VOLUME_DATA on {}", volume.path());
    let mut out = [0u8; VOLUME_DATA_LEN];
    let mut returned = 0u32;
    // SAFETY: the handle is owned by `volume`, which outlives the call. No input buffer is
    // passed; the output pointer and length describe `out`, and `returned` is a valid
    // out-pointer. The call is synchronous (no OVERLAPPED).
    let result = unsafe {
        DeviceIoControl(
            volume.raw(),
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
        Err(e) if e.code() == ERROR_INVALID_FUNCTION.to_hresult() => {
            return Err(ScanError::NotNtfs);
        }
        Err(e) => return Err(ScanError::Io(context(step, win32_error(e)))),
    }
    let len = usize::try_from(returned).map_or(out.len(), |n| n.min(out.len()));
    parse_volume_data(&out[..len])
        .map_err(|e| ScanError::BadLayout(format!("{step} returned {len} bytes: {e}")))
}

/// Extents of `$MFT`: from `FSCTL_GET_RETRIEVAL_POINTERS`, else from the data runs of
/// record 0 (which may miss runs stored in extension records of a very fragmented MFT).
fn mft_extents(letter: char, volume: &Volume, geo: &Geometry) -> Result<Vec<MftExtent>, ScanError> {
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
pub(crate) fn retrieval_pointers(letter: char) -> Result<Vec<MftExtent>, ScanError> {
    let path = format!(r"{letter}:\$MFT");
    let mft = OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES.0)
        .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0)
        .open(&path)
        .map_err(|e| ScanError::Io(context(format!("open {path}"), e)))?;

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
                HANDLE(mft.as_raw_handle()),
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
            Err(e) => {
                return Err(ScanError::Io(context(
                    format!("FSCTL_GET_RETRIEVAL_POINTERS on {path} from VCN {next_vcn}"),
                    win32_error(e),
                )));
            }
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
pub(crate) fn record_zero_extents(
    volume: &Volume,
    geo: &Geometry,
) -> Result<Vec<MftExtent>, ScanError> {
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
            volume.raw(),
            FSCTL_GET_NTFS_FILE_RECORD,
            Some(ptr::from_ref(&input).cast()),
            size_of::<NTFS_FILE_RECORD_INPUT_BUFFER>() as u32,
            Some(out.as_mut_ptr().cast()),
            out_len as u32,
            Some(&mut returned),
            None,
        )
    }
    .map_err(|e| {
        ScanError::Io(context(
            format!("FSCTL_GET_NTFS_FILE_RECORD(0) on {}", volume.path()),
            win32_error(e),
        ))
    })?;

    let len = usize::try_from(returned).map_or(out_len, |n| n.min(out_len));
    let bytes = words_to_bytes(&out, len);
    let (rec_no, record) = file_record_bytes(&bytes, geo.record)?;
    if rec_no != 0 {
        return Err(ScanError::BadLayout(format!(
            "FSCTL_GET_NTFS_FILE_RECORD(0) returned record {rec_no}"
        )));
    }
    let mut record = record.to_vec();
    match apply_fixups(&mut record, geo.sector_u32()) {
        // A mismatch means the record came back with its fixups already applied.
        Ok(()) | Err(NtfsError::FixupMismatch) => {}
        Err(e) => return Err(ScanError::BadLayout(format!("$MFT record 0 fixups: {e}"))),
    }
    let parsed = parse_record(&record, 0)
        .map_err(|e| ScanError::BadLayout(format!("$MFT record 0: {e}")))?
        .ok_or_else(|| ScanError::BadLayout("$MFT record is not in use".into()))?;
    let runs = parsed
        .mft_runs
        .ok_or_else(|| ScanError::BadLayout("$MFT record has no data runs".into()))?;
    Ok(extents_from_runs(&runs))
}

/// Reads every in-use record with `FSCTL_GET_NTFS_FILE_RECORD`, used when raw volume reads
/// are not possible. Several threads walk disjoint ranges of record numbers downward, each
/// with its own volume handle.
///
/// Returns the in-use records and the number of records that failed (bad records, failed
/// or invalid FSCTL answers).
fn read_by_records(
    letter: char,
    geo: &Geometry,
    reporter: &ProgressReporter<'_>,
    cancel: &CancelToken,
) -> Result<(Vec<FileRecord>, u64), ScanError> {
    let count = geo.mft_valid_len / geo.record;
    if count == 0 {
        return Err(ScanError::BadLayout("no MFT records to read".into()));
    }
    let threads = rayon::current_num_threads().clamp(1, MAX_RECORD_THREADS);
    let ranges = split_ranges(count, threads);

    std::thread::scope(|scope| {
        let workers: Vec<_> = ranges
            .iter()
            .map(|&(lo, hi)| {
                scope.spawn(move || read_record_range(letter, geo, lo, hi, reporter, cancel))
            })
            .collect();

        let mut records = Vec::new();
        let mut skipped = 0u64;
        let mut first_error = None;
        for worker in workers {
            match worker.join() {
                Ok(Ok((mut part, bad))) => {
                    records.append(&mut part);
                    skipped += bad;
                }
                Ok(Err(e)) => {
                    first_error.get_or_insert(e);
                }
                Err(_) => {
                    first_error.get_or_insert(ScanError::Io(io::Error::other(
                        "MFT record reader panicked",
                    )));
                }
            }
        }
        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        if let Some(e) = first_error {
            return Err(e);
        }
        Ok((records, skipped))
    })
}

/// One worker of [`read_by_records`]: walks `lo..=hi` downward on its own handle.
fn read_record_range(
    letter: char,
    geo: &Geometry,
    lo: u64,
    hi: u64,
    reporter: &ProgressReporter<'_>,
    cancel: &CancelToken,
) -> Result<(Vec<FileRecord>, u64), ScanError> {
    let volume = open_volume(letter, false)?;
    let mut fetcher = RecordFetcher::new(&volume, geo)?;
    let sector = geo.sector_u32();
    let mut records = Vec::new();
    let mut bad = 0u64;
    let mut unreported = 0u64;

    let counts = walk_down(
        lo,
        hi,
        |n| fetcher.fetch(n),
        |rec_no, mut bytes: Vec<u8>| {
            match parse_fetched_record(&mut bytes, rec_no, sector) {
                Slot::Used(record) => records.push(record),
                Slot::Bad => bad += 1,
                Slot::Free => {}
            }
            unreported += geo.record;
            if unreported >= RECORD_PROGRESS_BYTES {
                reporter.tick(0, 0, unreported, Phase::Reading);
                unreported = 0;
            }
        },
        || cancel.is_cancelled(),
    )
    .map_err(|e| {
        ScanError::Io(context(
            format!("reading MFT records {lo}..={hi} one by one"),
            e,
        ))
    })?;
    reporter.tick(0, 0, unreported, Phase::Reading);
    Ok((records, bad + counts.failed))
}

/// Issues `FSCTL_GET_NTFS_FILE_RECORD` calls on one handle, reusing its output buffer.
struct RecordFetcher<'a> {
    volume: &'a Volume,
    record_size: u64,
    /// `u64` words keep the output 8-byte aligned.
    out: Vec<u64>,
}

impl<'a> RecordFetcher<'a> {
    fn new(volume: &'a Volume, geo: &Geometry) -> Result<Self, ScanError> {
        let record_len = usize::try_from(geo.record)
            .map_err(|_| ScanError::BadLayout("record too large".into()))?;
        let words = (FILE_RECORD_HEADER + record_len).div_ceil(size_of::<u64>());
        Ok(Self {
            volume,
            record_size: geo.record,
            out: vec![0u64; words],
        })
    }

    /// Asks for record `n`; NTFS answers with the highest in-use record `<= n`. Returns that
    /// record's number and a copy of its bytes (fixups normally already applied).
    fn fetch(&mut self, n: u64) -> io::Result<(u64, Vec<u8>)> {
        let step = || format!("FSCTL_GET_NTFS_FILE_RECORD({n}) on {}", self.volume.path());
        let input = NTFS_FILE_RECORD_INPUT_BUFFER {
            FileReferenceNumber: i64::try_from(n).map_err(|_| {
                context(
                    step(),
                    io::Error::new(io::ErrorKind::InvalidInput, "record number"),
                )
            })?,
        };
        let out_len = self.out.len() * size_of::<u64>();
        let mut returned = 0u32;
        // SAFETY: the handle is owned by `self.volume`, which outlives the call. The input
        // pointer and size describe `input`; the output pointer and size describe `self.out`;
        // `returned` is a valid out-pointer. The call is synchronous (no OVERLAPPED).
        unsafe {
            DeviceIoControl(
                self.volume.raw(),
                FSCTL_GET_NTFS_FILE_RECORD,
                Some(ptr::from_ref(&input).cast()),
                size_of::<NTFS_FILE_RECORD_INPUT_BUFFER>() as u32,
                Some(self.out.as_mut_ptr().cast()),
                out_len as u32,
                Some(&mut returned),
                None,
            )
        }
        .map_err(|e| context(step(), win32_error(e)))?;

        let len = usize::try_from(returned).map_or(out_len, |r| r.min(out_len));
        let bytes = words_to_bytes(&self.out, len);
        let (rec_no, record) = file_record_bytes(&bytes, self.record_size)
            .map_err(|e| context(step(), io::Error::other(e.to_string())))?;
        Ok((rec_no, record.to_vec()))
    }
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
    volume: &Volume,
    plan: &[BlockPlan],
    geo: &Geometry,
    reporter: &ProgressReporter<'_>,
    cancel: &CancelToken,
) -> Result<(Vec<FileRecord>, u64), ScanError> {
    let align = read_alignment(geo);
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
    volume: &'a Volume,
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
            let (start, data) = aligned(&mut buf, self.align, planned.len)?;
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
///
/// Errors name the MFT block as well as the failed read's volume offset and length.
pub(crate) fn fill_block(volume: &Volume, planned: &BlockPlan, data: &mut [u8]) -> io::Result<()> {
    let step = || {
        format!(
            "MFT block at MFT byte {:#x} ({} bytes, {} segments)",
            planned.mft_offset,
            planned.len,
            planned.segments.len()
        )
    };
    let bad_plan = || context(step(), io::Error::other("inconsistent read plan"));
    let mut pos = 0;
    for seg in &planned.segments {
        let end = seg.buf_offset.checked_add(seg.len).ok_or_else(bad_plan)?;
        data.get_mut(pos..seg.buf_offset)
            .ok_or_else(bad_plan)?
            .fill(0);
        let target = data.get_mut(seg.buf_offset..end).ok_or_else(bad_plan)?;
        volume
            .read_exact_at(target, seg.volume_offset)
            .map_err(|e| context(step(), e))?;
        pos = end;
    }
    data.get_mut(pos..).ok_or_else(bad_plan)?.fill(0);
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
    fn fake_volume(records: &[(u64, Vec<u8>)]) -> (tempfile::NamedTempFile, Volume) {
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
        let file = Volume::open_path(tmp.path(), false).unwrap();
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
        let Err(ScanError::Io(err)) = result else {
            panic!("expected an I/O error, got {result:?}");
        };
        // The error locates itself: block, volume offset (cluster 100 = 0x19000) and length.
        let msg = err.to_string();
        assert!(msg.contains("MFT block at MFT byte 0x0"), "{msg}");
        assert!(msg.contains("at volume offset 0x19000"), "{msg}");
        assert!(msg.contains("read 2048 bytes"), "{msg}");
    }

    /// MFT records 0-1 at cluster 10 (readable), then records 2-3 at cluster 100, past the
    /// end of the fake volume: the first block reads fine and the second one fails.
    fn plan_failing_at_second_block() -> (Geometry, Vec<BlockPlan>) {
        let extents = [
            MftExtent {
                vcn: 0,
                lcn: 10,
                clusters: 2,
            },
            MftExtent {
                vcn: 2,
                lcn: 100,
                clusters: 2,
            },
        ];
        let geo = Geometry {
            total_clusters: 1000,
            ..geometry(4 * 1024)
        };
        let plan = plan_blocks(&extents, &geo, 2048).unwrap();
        assert_eq!(plan.len(), 2);
        (geo, plan)
    }

    #[test]
    fn mid_scan_read_failure_falls_back_to_records() {
        let (_tmp, volume) = fake_volume(&[
            (10 * 1024, raw_record(RECORD, true)),
            (11 * 1024, raw_record(RECORD, true)),
        ]);
        let (geo, plan) = plan_failing_at_second_block();
        // The block path can start: the first block reads.
        test_read(&volume, &plan[0], &geo).unwrap();

        let reporter = ProgressReporter::new(&|_| {});
        let cancel = CancelToken::new();
        let mut fallback_ran = false;
        let (records, skipped, reason) = read_with_fallback(
            Ok(()),
            |()| read_and_parse(&volume, &plan, &geo, &reporter, &cancel),
            || {
                fallback_ran = true;
                Ok((Vec::new(), 7))
            },
        )
        .unwrap();

        assert!(fallback_ran);
        assert!(records.is_empty());
        assert_eq!(
            skipped, 7,
            "the record-by-record result is used for the whole scan"
        );
        let reason = reason.expect("a fallback reason is set");
        assert!(
            reason.starts_with("raw MFT read failed mid-scan ("),
            "{reason}"
        );
        assert!(reason.contains("MFT block at MFT byte 0x800"), "{reason}");
        assert!(reason.ends_with("; read records one by one"), "{reason}");
    }

    #[test]
    fn blocked_raw_reads_keep_the_reason() {
        let blocked: Result<(), ScanError> = Err(ScanError::Io(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "access is denied",
        )));
        let (_, _, reason) = read_with_fallback(
            blocked,
            |()| panic!("block reads must not run"),
            || Ok((Vec::new(), 0)),
        )
        .unwrap();
        let reason = reason.expect("a fallback reason is set");
        assert!(reason.starts_with("raw MFT reads blocked ("), "{reason}");
        assert!(reason.contains("access is denied"), "{reason}");
    }

    #[test]
    fn block_reads_that_succeed_have_no_fallback_reason() {
        let (_, skipped, reason) = read_with_fallback(
            Ok(()),
            |()| Ok((Vec::new(), 3)),
            || panic!("record reads must not run"),
        )
        .unwrap();
        assert_eq!((skipped, reason), (3, None));
    }

    #[test]
    fn cancelled_block_reads_do_not_fall_back() {
        let result = read_with_fallback(
            Ok(()),
            |()| Err(ScanError::Cancelled),
            || panic!("record reads must not run"),
        );
        assert!(matches!(result, Err(ScanError::Cancelled)));
    }

    #[test]
    fn fallback_errors_are_returned() {
        let result = read_with_fallback(
            Err::<(), _>(ScanError::BadLayout("no extents".into())),
            |()| unreachable!(),
            || Err(ScanError::AccessDenied),
        );
        assert!(matches!(result, Err(ScanError::AccessDenied)));
    }

    #[test]
    fn unbuffered_reads_work_on_a_plain_file() {
        // Same flags and read path as on a real volume, with 4 KiB-aligned offsets.
        let marker = raw_record(4096, true);
        let (tmp, _) = fake_volume(&[(8192, marker.clone())]);
        let volume = Volume::open_path(tmp.path(), true).unwrap();
        let geo = Geometry {
            cluster: 4096,
            record: 4096,
            total_clusters: 16,
            ..geometry(16 * 1024)
        };
        let extents = [MftExtent {
            vcn: 0,
            lcn: 2,
            clusters: 4,
        }];
        let plan = plan_blocks(&extents, &geo, block_size(&geo)).unwrap();
        test_read(&volume, &plan[0], &geo).unwrap();

        let align = read_alignment(&geo);
        let mut buf = vec![0u8; 4096 + align];
        let (_, data) = aligned(&mut buf, align, 4096).unwrap();
        volume.read_exact_at(data, 8192).unwrap();
        assert_eq!(data, &marker[..]);
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
