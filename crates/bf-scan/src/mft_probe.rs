//! Step-by-step diagnostics of the MFT scanner, for troubleshooting on an elevated machine.
//! Used by `examples/mft_probe.rs`; not part of the stable API.

use std::fmt::Display;
use std::fs::OpenOptions;
use std::os::windows::fs::{FileExt, OpenOptionsExt};

use windows::Win32::Storage::FileSystem::{
    FILE_FLAG_NO_BUFFERING, FILE_SHARE_READ, FILE_SHARE_WRITE,
};

use bf_core::Tree;

use crate::mft::{
    ReadMode, aligned, block_reader, check_letter, open_volume, read_alignment,
    record_zero_extents, retrieval_pointers, scan_volume_with, test_read, volume_data,
};
use crate::mft_layout::{Geometry, MftExtent, block_size, plan_blocks};
use crate::volumes::filesystem_name;
use crate::{CancelToken, ProgressFn, ScanError, ScanStats, is_elevated};

/// Runs the MFT scanner in record-by-record mode only (`FSCTL_GET_NTFS_FILE_RECORD`), the
/// fallback used when raw volume reads fail. For the probe example.
#[doc(hidden)]
pub fn probe_mft_records(
    letter: char,
    progress: ProgressFn<'_>,
    cancel: &CancelToken,
) -> Result<(Tree, ScanStats), ScanError> {
    scan_volume_with(letter, progress, cancel, ReadMode::RecordsOnly)
}

/// Logs `what` and the outcome of `result`, then passes the result on.
fn step<T, E: Display>(
    log: &mut dyn FnMut(String),
    what: &str,
    result: Result<T, E>,
) -> Result<T, E> {
    match &result {
        Ok(_) => log(format!("[ok]   {what}")),
        Err(e) => log(format!("[FAIL] {what}: {e}")),
    }
    result
}

fn log_extents(log: &mut dyn FnMut(String), extents: &[MftExtent]) {
    log(format!("       {} extents", extents.len()));
    for e in extents.iter().take(3) {
        log(format!(
            "       vcn {} lcn {} clusters {}",
            e.vcn, e.lcn, e.clusters
        ));
    }
}

/// Runs each step of the MFT scanner on drive `letter` and describes it through `log`:
/// volume data, `$MFT` extents (both sources), the read plan, and test reads of the first
/// MFT bytes with buffered and unbuffered handles, through both `ReadFile` and
/// `std::fs::File::seek_read`. Returns the first error that stops the probe.
#[doc(hidden)]
pub fn probe_mft(letter: char, log: &mut dyn FnMut(String)) -> Result<(), ScanError> {
    let letter = check_letter(letter)?;
    log(format!("drive {letter}: elevated={}", is_elevated()));
    log(format!("filesystem: {:?}", filesystem_name(letter)));

    let volume = step(log, "open volume (unbuffered)", open_volume(letter, true))?;
    let info = step(log, "FSCTL_GET_NTFS_VOLUME_DATA", volume_data(&volume))?;
    log(format!(
        "       sector {} cluster {} record {} mft_valid_len {} mft_start_lcn {} total_clusters {}",
        info.bytes_per_sector,
        info.bytes_per_cluster,
        info.bytes_per_record,
        info.mft_valid_data_length,
        info.mft_start_lcn,
        info.total_clusters
    ));
    let geo = step(log, "geometry checks", Geometry::new(&info))?;

    match block_reader(letter, open_volume(letter, true)?, &geo) {
        Ok((_, plan)) => log(format!(
            "[ok]   block reader: {} blocks; the scanner reads the MFT in raw blocks",
            plan.len()
        )),
        Err(e) => log(format!(
            "[FAIL] block reader: {e}
       -> the scanner falls back to              FSCTL_GET_NTFS_FILE_RECORD record by record"
        )),
    }

    let rp = step(
        log,
        "FSCTL_GET_RETRIEVAL_POINTERS on $MFT",
        retrieval_pointers(letter),
    );
    if let Ok(extents) = &rp {
        log_extents(log, extents);
    }
    let r0 = step(
        log,
        "FSCTL_GET_NTFS_FILE_RECORD(0) runs",
        record_zero_extents(&volume, &geo),
    );
    if let Ok(extents) = &r0 {
        log_extents(log, extents);
    }
    let extents = match (rp, r0) {
        (Ok(e), _) if !e.is_empty() => e,
        (_, Ok(e)) => e,
        (_, Err(_)) => {
            log("       no $MFT extents: raw block reads cannot be planned".into());
            return Ok(());
        }
    };

    let block = block_size(&geo);
    let Ok(plan) = step(log, "read plan", plan_blocks(&extents, &geo, block)) else {
        return Ok(());
    };
    let total: usize = plan.iter().map(|b| b.len).sum();
    log(format!(
        "       {} blocks of up to {block} bytes, {total} bytes in all",
        plan.len()
    ));
    let Some(first) = plan.first() else {
        return Ok(());
    };
    for s in first.segments.iter().take(3) {
        log(format!(
            "       first block segment: volume offset {:#x} len {} (buffer offset {})",
            s.volume_offset, s.len, s.buf_offset
        ));
    }
    let Some(seg) = first.segments.first().copied() else {
        log("       first block has no segments".into());
        return Ok(());
    };
    drop(volume);

    // Test reads at the start of the MFT, growing in size, with both handle kinds.
    let sector = usize::try_from(geo.sector).unwrap_or(512);
    let record = usize::try_from(geo.record).unwrap_or(1024);
    let mut sizes: Vec<usize> = [sector, record, 64 << 10, 1 << 20, seg.len]
        .into_iter()
        .filter(|&n| n <= seg.len && n % sector == 0)
        .collect();
    sizes.sort_unstable();
    sizes.dedup();
    let align = read_alignment(&geo);
    for unbuffered in [true, false] {
        let mode = if unbuffered { "unbuffered" } else { "buffered" };
        let Ok(vol) = step(
            log,
            &format!("open ({mode}) for test reads"),
            open_volume(letter, unbuffered),
        ) else {
            continue;
        };
        for &len in &sizes {
            let mut buf = vec![0u8; len + align];
            let result = aligned(&mut buf, align, len)
                .and_then(|(_, data)| vol.read_exact_at(data, seg.volume_offset));
            let _ = step(
                log,
                &format!("ReadFile {mode}: {len} bytes at {:#x}", seg.volume_offset),
                result,
            );
        }
        let _ = step(
            log,
            &format!("first block ({} bytes) via fill_block, {mode}", first.len),
            test_read(&vol, first, &geo),
        );

        // The path used before fix round 1: std's seek_read (NtReadFile with a byte offset).
        let len = seg.len.min(1 << 20);
        let flags = if unbuffered {
            FILE_FLAG_NO_BUFFERING.0
        } else {
            0
        };
        let std_read = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0)
            .custom_flags(flags)
            .open(format!(r"\\.\{letter}:"))
            .and_then(|file| {
                let mut buf = vec![0u8; len + align];
                let (_, data) = aligned(&mut buf, align, len)?;
                file.seek_read(data, seg.volume_offset)
            });
        match std_read {
            Ok(n) => log(format!(
                "[ok]   std seek_read {mode}: {n} of {len} bytes at {:#x}",
                seg.volume_offset
            )),
            Err(e) => log(format!(
                "[FAIL] std seek_read {mode}: {len} bytes at {:#x}: {e}",
                seg.volume_offset
            )),
        }
    }
    Ok(())
}
