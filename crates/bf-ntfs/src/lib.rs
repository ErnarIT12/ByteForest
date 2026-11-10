//! ByteForest NTFS parsing: volume data, FILE record header and fixups.
//!
//! Everything here reads untrusted bytes from disk, so no function panics:
//! every access goes through the bounds-checked readers below.

mod error;
mod record;
mod volume;

pub use error::NtfsError;
pub use record::{RecordHeader, apply_fixups, parse_header};
pub use volume::{VolumeInfo, parse_volume_data};

/// Returns `len` bytes at `off`, or `Truncated` if they are not all in `buf`.
fn bytes_at(buf: &[u8], off: usize, len: usize) -> Result<&[u8], NtfsError> {
    let end = off.checked_add(len).ok_or(NtfsError::Truncated)?;
    buf.get(off..end).ok_or(NtfsError::Truncated)
}

/// Reads a little-endian `u16` at `off`.
fn u16_at(buf: &[u8], off: usize) -> Result<u16, NtfsError> {
    let b = bytes_at(buf, off, 2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

/// Reads a little-endian `u32` at `off`.
fn u32_at(buf: &[u8], off: usize) -> Result<u32, NtfsError> {
    let b = bytes_at(buf, off, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Reads a little-endian `u64` at `off`.
fn u64_at(buf: &[u8], off: usize) -> Result<u64, NtfsError> {
    let b = bytes_at(buf, off, 8)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
