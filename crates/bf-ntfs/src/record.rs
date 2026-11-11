use crate::attr::{
    Attrs, TYPE_DATA, TYPE_FILE_NAME, TYPE_STANDARD_INFORMATION, parse_data, parse_file_name,
    parse_std_info,
};
use crate::runs::{Extent, decode_runs};
use crate::{NtfsError, bytes_at, u16_at, u32_at, u64_at};

/// Flag: the record is in use.
const FLAG_IN_USE: u16 = 0x1;
/// Flag: the record is a directory.
const FLAG_DIR: u16 = 0x2;
/// The base-record reference keeps its record number in the low 48 bits.
const RECORD_NUMBER_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

/// Record number of `$BadClus`, whose `$Bad` stream spans the whole volume.
const BADCLUS_RECORD: u64 = 8;
/// Name of the `$BadClus` stream that maps bad clusters.
const BADCLUS_STREAM: &str = "$Bad";

/// End of the fixed part of the FILE record header; attributes start after it.
const HEADER_END: usize = 42;
/// `$FILE_NAME` namespaces we keep: POSIX, Win32, and the shared Win32+DOS name.
/// Namespace 2 (a DOS 8.3 short name) only duplicates a Win32 name.
const KEPT_NAMESPACES: [u8; 3] = [0, 1, 3];

/// Fields of the FILE record header that the scanner needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordHeader {
    pub in_use: bool,
    pub is_dir: bool,
    /// Record number of the base record (0 if this is a base record itself).
    pub base_record: u64,
    pub link_count: u16,
    /// Offset of the first attribute inside the record.
    pub first_attr_offset: u16,
    /// Bytes of the record actually in use.
    pub used_size: u32,
}

/// Fails with `BadMagic` unless the record starts with `FILE`.
///
/// `BAAD` (a record NTFS marked as failed) is also rejected.
fn check_magic(record: &[u8]) -> Result<(), NtfsError> {
    if bytes_at(record, 0, 4)? == b"FILE" {
        Ok(())
    } else {
        Err(NtfsError::BadMagic)
    }
}

/// Parses the FILE record header. Call `apply_fixups` first on raw disk data.
pub fn parse_header(record: &[u8]) -> Result<RecordHeader, NtfsError> {
    check_magic(record)?;
    let flags = u16_at(record, 22)?;
    Ok(RecordHeader {
        in_use: flags & FLAG_IN_USE != 0,
        is_dir: flags & FLAG_DIR != 0,
        base_record: u64_at(record, 32)? & RECORD_NUMBER_MASK,
        link_count: u16_at(record, 18)?,
        first_attr_offset: u16_at(record, 20)?,
        used_size: u32_at(record, 24)?,
    })
}

/// Undoes the update sequence protection of a FILE record, in place.
///
/// NTFS overwrites the last two bytes of every sector with an update sequence
/// number (`usa[0]`) and stores the real bytes in `usa[1..]`. For each sector
/// the tail must equal `usa[0]` and is replaced by its saved value.
///
/// All sectors are checked before anything is written, so on error the record
/// is left unchanged.
pub fn apply_fixups(record: &mut [u8], bytes_per_sector: u32) -> Result<(), NtfsError> {
    check_magic(record)?;
    let sector = usize::try_from(bytes_per_sector).map_err(|_| NtfsError::Truncated)?;
    if sector < 2 || record.is_empty() || !record.len().is_multiple_of(sector) {
        return Err(NtfsError::Truncated);
    }
    let sectors = record.len() / sector;

    let usa_offset = usize::from(u16_at(record, 4)?);
    let usa_count = usize::from(u16_at(record, 6)?);
    // One USA entry for the sequence number plus one per sector.
    if usa_count == 0 || usa_count - 1 != sectors {
        return Err(NtfsError::Truncated);
    }
    let usa_len = usa_count.checked_mul(2).ok_or(NtfsError::Truncated)?;
    let usa = bytes_at(record, usa_offset, usa_len)?;
    let seq = [usa[0], usa[1]];

    // Pass 1: verify every sector tail; collect the saved values.
    let mut saved = Vec::with_capacity(sectors);
    for i in 1..usa_count {
        let tail = bytes_at(record, i * sector - 2, 2)?;
        if tail != seq {
            return Err(NtfsError::FixupMismatch);
        }
        saved.push([usa[2 * i], usa[2 * i + 1]]);
    }

    // Pass 2: restore the original bytes.
    for (i, original) in saved.iter().enumerate() {
        let tail = (i + 1) * sector - 2;
        record[tail..tail + 2].copy_from_slice(original);
    }
    Ok(())
}

/// One name (hard link) of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileName {
    /// Record number of the parent directory.
    pub parent: u64,
    /// `$FILE_NAME` namespace: 0 POSIX, 1 Win32, 3 Win32 and DOS.
    pub namespace: u8,
    pub name: String,
}

/// What the scanner needs from one FILE record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecord {
    pub rec_no: u64,
    /// Record number of the base record (0 if this record is a base record).
    pub base_record: u64,
    pub is_dir: bool,
    /// Last modification time (FILETIME); 0 if the record has no
    /// `$STANDARD_INFORMATION` (extension records).
    pub mtime: i64,
    /// DOS file attribute flags; 0 if the record has no `$STANDARD_INFORMATION`.
    pub file_attributes: u32,
    /// Names without the DOS 8.3 ones. Several entries mean hard links.
    pub names: Vec<FileName>,
    /// Logical size of the unnamed `$DATA` stream only. Named streams (alternate
    /// data streams, `WofCompressedData`, `Zone.Identifier`, `$UsnJrnl:$J`...)
    /// count only in `alloc`, and `$BadClus:$Bad` counts nowhere.
    pub size: u64,
    /// Allocated size, summed over all `$DATA` streams: the compressed size for
    /// sparse or compressed streams, else the allocated size. Resident data counts
    /// 0 because it lives inside the MFT record.
    pub alloc: u64,
    /// Runs of the unnamed `$DATA` of `$MFT` (record 0 only).
    pub mft_runs: Option<Vec<Extent>>,
}

/// Parses an in-use FILE record after `apply_fixups`.
///
/// Returns `Ok(None)` when the record is not in use. Attributes other than
/// `$STANDARD_INFORMATION`, `$FILE_NAME` and `$DATA` are skipped, including
/// `$ATTRIBUTE_LIST`: the caller merges extension records through
/// `base_record`. Every offset and length is checked against the used part of
/// the record, so corrupt input yields an error, never a panic.
pub fn parse_record(record: &[u8], rec_no: u64) -> Result<Option<FileRecord>, NtfsError> {
    let header = parse_header(record)?;
    if !header.in_use {
        return Ok(None);
    }

    // `parse_header` does not validate these two fields: bound them here.
    let used = usize::try_from(header.used_size).unwrap_or(usize::MAX);
    let region = &record[..used.min(record.len())];
    let first = usize::from(header.first_attr_offset);
    if first < HEADER_END || first >= region.len() {
        return Err(NtfsError::BadAttribute(first));
    }

    let mut file = FileRecord {
        rec_no,
        base_record: header.base_record,
        is_dir: header.is_dir,
        mtime: 0,
        file_attributes: 0,
        names: Vec::new(),
        size: 0,
        alloc: 0,
        mft_runs: None,
    };
    let mut seen_std_info = false;

    for attr in Attrs::new(region, first) {
        let attr = attr?;
        let bad = |_: NtfsError| NtfsError::BadAttribute(attr.offset);
        match attr.type_code {
            TYPE_STANDARD_INFORMATION if !seen_std_info => {
                let info = parse_std_info(&attr).map_err(bad)?;
                file.mtime = info.mtime;
                file.file_attributes = info.file_attributes;
                seen_std_info = true;
            }
            TYPE_FILE_NAME => {
                let raw = parse_file_name(&attr).map_err(bad)?;
                let is_duplicate = file
                    .names
                    .iter()
                    .any(|n| n.parent == raw.parent && n.name == raw.name);
                if KEPT_NAMESPACES.contains(&raw.namespace) && !is_duplicate {
                    file.names.push(FileName {
                        parent: raw.parent,
                        namespace: raw.namespace,
                        name: raw.name,
                    });
                }
            }
            TYPE_DATA => {
                let data = parse_data(&attr).map_err(bad)?;
                // `$BadClus:$Bad` is as large as the volume (sparse except for bad clusters,
                // and often not even flagged sparse): it holds no data, so it counts nothing.
                // The stream may live in an extension record whose base is record 8.
                let in_badclus = rec_no == BADCLUS_RECORD || header.base_record == BADCLUS_RECORD;
                if in_badclus && data.named && attr.name_is(BADCLUS_STREAM).map_err(bad)? {
                    continue;
                }
                // Only the unnamed stream gives the logical size. Named streams count
                // only their allocation: a WOF-compressed file keeps its logical size in
                // a sparse unnamed stream and its data in `WofCompressedData`, and a
                // sparse stream like `$UsnJrnl:$J` has a logical size far beyond its data.
                if !data.named {
                    file.size = file.size.saturating_add(data.size);
                }
                file.alloc = file.alloc.saturating_add(data.alloc);
                if rec_no == 0
                    && !data.named
                    && file.mft_runs.is_none()
                    && let Some(runs) = data.runs
                {
                    file.mft_runs = Some(decode_runs(runs).map_err(bad)?);
                }
            }
            _ => {}
        }
    }
    Ok(Some(file))
}
