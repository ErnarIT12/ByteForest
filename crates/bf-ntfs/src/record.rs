use crate::{NtfsError, bytes_at, u16_at, u32_at, u64_at};

/// Flag: the record is in use.
const FLAG_IN_USE: u16 = 0x1;
/// Flag: the record is a directory.
const FLAG_DIR: u16 = 0x2;
/// The base-record reference keeps its record number in the low 48 bits.
const RECORD_NUMBER_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

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
