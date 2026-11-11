use crate::{NtfsError, bytes_at};

/// A contiguous run of clusters on the volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extent {
    /// First cluster of the run (logical cluster number).
    pub lcn: u64,
    /// Number of clusters in the run.
    pub clusters: u64,
}

/// Reads up to 8 bytes as an unsigned little-endian integer.
fn le_unsigned(bytes: &[u8]) -> u64 {
    let mut raw = [0u8; 8];
    raw[..bytes.len()].copy_from_slice(bytes);
    u64::from_le_bytes(raw)
}

/// Reads up to 8 bytes as a signed little-endian integer (sign-extended).
fn le_signed(bytes: &[u8]) -> i64 {
    let negative = bytes.last().is_some_and(|top| top & 0x80 != 0);
    let mut raw = if negative { [0xFF; 8] } else { [0u8; 8] };
    raw[..bytes.len()].copy_from_slice(bytes);
    i64::from_le_bytes(raw)
}

/// Decodes an NTFS data-run list (the "mapping pairs").
///
/// Each run starts with a header byte: the low nibble is the size of the
/// length field, the high nibble the size of the offset field. The length is
/// unsigned; the offset is signed and relative to the previous run's LCN. A
/// run with no offset field is sparse (a hole): it has no clusters on disk, so
/// it produces no `Extent` and leaves the current LCN unchanged. A header byte
/// of `0x00` ends the list.
///
/// Anything malformed, including a list without its terminator, is
/// `Truncated`.
pub fn decode_runs(buf: &[u8]) -> Result<Vec<Extent>, NtfsError> {
    let mut extents = Vec::new();
    let mut pos = 0usize;
    let mut lcn = 0i64;
    loop {
        let header = *buf.get(pos).ok_or(NtfsError::Truncated)?;
        pos += 1;
        if header == 0 {
            return Ok(extents);
        }
        let length_size = usize::from(header & 0x0F);
        let offset_size = usize::from(header >> 4);
        if length_size == 0 || length_size > 8 || offset_size > 8 {
            return Err(NtfsError::Truncated);
        }

        let clusters = le_unsigned(bytes_at(buf, pos, length_size)?);
        pos += length_size;
        if offset_size == 0 {
            continue; // sparse run
        }
        let delta = le_signed(bytes_at(buf, pos, offset_size)?);
        pos += offset_size;

        lcn = lcn.checked_add(delta).ok_or(NtfsError::Truncated)?;
        let start = u64::try_from(lcn).map_err(|_| NtfsError::Truncated)?;
        extents.push(Extent {
            lcn: start,
            clusters,
        });
    }
}
